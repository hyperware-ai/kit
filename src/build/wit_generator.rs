use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use color_eyre::{
    eyre::{bail, eyre, WrapErr},
    Result,
};
use syn::{self, Attribute, ImplItem, Item, Type};
use toml::Value;
use walkdir::WalkDir;

// Helper functions for naming conventions
fn to_kebab_case(s: &str) -> String {
    // First, handle the case where the input has underscores
    if s.contains('_') {
        return s.replace('_', "-");
    }

    let mut result = String::with_capacity(s.len() + 5); // Extra capacity for hyphens
    let chars: Vec<char> = s.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            // Add hyphen if:
            // 1. Not the first character
            // 2. Previous character is lowercase
            // 3. Or next character is lowercase (to handle acronyms like HTML)
            if i > 0
                && (chars[i - 1].is_lowercase()
                    || (i < chars.len() - 1 && chars[i + 1].is_lowercase()))
            {
                result.push('-');
            }
            result.push(c.to_lowercase().next().unwrap());
        } else {
            result.push(c);
        }
    }

    result
}

// Validates a name doesn't contain numbers or "stream"
fn validate_name(name: &str, kind: &str) -> Result<()> {
    // Check for numbers
    if name.chars().any(|c| c.is_digit(10)) {
        bail!(
            "Error: {} name '{}' contains numbers, which is not allowed",
            kind,
            name
        );
    }

    // Check for "stream"
    if name.to_lowercase().contains("stream") {
        bail!(
            "Error: {} name '{}' contains 'stream', which is not allowed",
            kind,
            name
        );
    }

    Ok(())
}

// Remove "State" suffix from a name
fn remove_state_suffix(name: &str) -> String {
    if name.ends_with("State") {
        let len = name.len();
        return name[0..len - 5].to_string();
    }
    name.to_string()
}

// Extract wit_world from the #[hyperprocess] attribute using the format in the debug representation
fn extract_wit_world(attrs: &[Attribute]) -> Result<String> {
    for attr in attrs {
        if attr.path().is_ident("hyperprocess") {
            // Convert attribute to string representation
            let attr_str = format!("{:?}", attr);
            println!("Attribute string: {}", attr_str);

            // Look for wit_world in the attribute string
            if let Some(pos) = attr_str.find("wit_world") {
                println!("Found wit_world at position {}", pos);

                // Find the literal value after wit_world by looking for lit: "value"
                let lit_pattern = "lit: \"";
                if let Some(lit_pos) = attr_str[pos..].find(lit_pattern) {
                    let start_pos = pos + lit_pos + lit_pattern.len();

                    // Find the closing quote of the literal
                    if let Some(quote_pos) = attr_str[start_pos..].find('\"') {
                        let world_name = &attr_str[start_pos..(start_pos + quote_pos)];
                        println!("Extracted wit_world: {}", world_name);
                        return Ok(world_name.to_string());
                    }
                }
            }
        }
    }
    bail!("wit_world not found in hyperprocess attribute")
}

// Convert Rust type to WIT type, including downstream types
fn rust_type_to_wit(ty: &Type, used_types: &mut HashSet<String>) -> Result<String> {
    match ty {
        Type::Path(type_path) => {
            if type_path.path.segments.is_empty() {
                return Err(eyre!("Failed to parse path type: {ty:?}"));
            }

            let ident = &type_path.path.segments.last().unwrap().ident;
            let type_name = ident.to_string();

            match type_name.as_str() {
                "i8" => Ok("s8".to_string()),
                "u8" => Ok("u8".to_string()),
                "i16" => Ok("s16".to_string()),
                "u16" => Ok("u16".to_string()),
                "i32" => Ok("s32".to_string()),
                "u32" => Ok("u32".to_string()),
                "i64" => Ok("s64".to_string()),
                "u64" => Ok("u64".to_string()),
                "f32" => Ok("f32".to_string()),
                "f64" => Ok("f64".to_string()),
                "String" => Ok("string".to_string()),
                "bool" => Ok("bool".to_string()),
                "Vec" => {
                    if let syn::PathArguments::AngleBracketed(args) =
                        &type_path.path.segments.last().unwrap().arguments
                    {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            let inner_type = rust_type_to_wit(inner_ty, used_types)?;
                            Ok(format!("list<{}>", inner_type))
                        } else {
                            Err(eyre!("Failed to parse Vec inner type"))
                        }
                    } else {
                        Err(eyre!("Failed to parse Vec inner type!"))
                    }
                }
                "Option" => {
                    if let syn::PathArguments::AngleBracketed(args) =
                        &type_path.path.segments.last().unwrap().arguments
                    {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            let inner_type = rust_type_to_wit(inner_ty, used_types)?;
                            Ok(format!("option<{}>", inner_type))
                        } else {
                            Err(eyre!("Failed to parse Option inner type"))
                        }
                    } else {
                        Err(eyre!("Failed to parse Option inner type!"))
                    }
                }
                "Result" => {
                    if let syn::PathArguments::AngleBracketed(args) =
                        &type_path.path.segments.last().unwrap().arguments
                    {
                        if args.args.len() >= 2 {
                            if let (
                                Some(syn::GenericArgument::Type(ok_ty)),
                                Some(syn::GenericArgument::Type(err_ty)),
                            ) = (args.args.first(), args.args.get(1))
                            {
                                let ok_type = rust_type_to_wit(ok_ty, used_types)?;
                                let err_type = rust_type_to_wit(err_ty, used_types)?;
                                Ok(format!("result<{}, {}>", ok_type, err_type))
                            } else {
                                Err(eyre!("Failed to parse Result generic arguments"))
                            }
                        } else {
                            Err(eyre!("Result requires two type arguments"))
                        }
                    } else {
                        Err(eyre!("Failed to parse Result type arguments"))
                    }
                }
                // TODO: fix and enable
                //"HashMap" | "BTreeMap" => {
                //    if let syn::PathArguments::AngleBracketed(args) =
                //        &type_path.path.segments.last().unwrap().arguments
                //    {
                //        if args.args.len() >= 2 {
                //            if let (
                //                Some(syn::GenericArgument::Type(key_ty)),
                //                Some(syn::GenericArgument::Type(val_ty)),
                //            ) = (args.args.first(), args.args.get(1))
                //            {
                //                let key_type = rust_type_to_wit(key_ty, used_types)?;
                //                let val_type = rust_type_to_wit(val_ty, used_types)?;
                //                // For HashMaps, we'll generate a list of tuples where each tuple contains a key and value
                //                Ok(format!("list<tuple<{}, {}>>", key_type, val_type))
                //            } else {
                //                Ok("list<tuple<string, any>>".to_string())
                //            }
                //        } else {
                //            Ok("list<tuple<string, any>>".to_string())
                //        }
                //    } else {
                //        Ok("list<tuple<string, any>>".to_string())
                //    }
                //}
                custom => {
                    // Validate custom type name
                    validate_name(custom, "Type")?;

                    // Convert custom type to kebab-case and add to used types
                    let kebab_custom = to_kebab_case(custom);
                    used_types.insert(kebab_custom.clone());
                    Ok(kebab_custom)
                }
            }
        }
        Type::Reference(type_ref) => {
            // Handle references by using the underlying type
            rust_type_to_wit(&type_ref.elem, used_types)
        }
        Type::Tuple(type_tuple) => {
            if type_tuple.elems.is_empty() {
                // Empty tuple is unit in WIT
                Ok("unit".to_string())
            } else {
                // Create a tuple representation in WIT
                let mut elem_types = Vec::new();
                for elem in &type_tuple.elems {
                    elem_types.push(rust_type_to_wit(elem, used_types)?);
                }
                Ok(format!("tuple<{}>", elem_types.join(", ")))
            }
        }
        _ => return Err(eyre!("Failed to parse type: {ty:?}")),
    }
}

// Find all Rust files in a crate directory
fn find_rust_files(crate_path: &Path) -> Vec<PathBuf> {
    let mut rust_files = Vec::new();
    let src_dir = crate_path.join("src");

    println!("Finding Rust files in {}", src_dir.display());

    if !src_dir.exists() || !src_dir.is_dir() {
        println!("No src directory found at {}", src_dir.display());
        return rust_files;
    }

    for entry in WalkDir::new(src_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
            println!("Found Rust file: {}", path.display());
            rust_files.push(path.to_path_buf());
        }
    }

    println!("Found {} Rust files", rust_files.len());
    rust_files
}

// Collect type definitions (structs and enums) from a file
fn collect_type_definitions_from_file(file_path: &Path) -> Result<HashMap<String, String>> {
    println!(
        "Collecting type definitions from file: {}",
        file_path.display()
    );

    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

    let ast = syn::parse_file(&content)
        .with_context(|| format!("Failed to parse file: {}", file_path.display()))?;

    let mut type_defs = HashMap::new();

    for item in &ast.items {
        match item {
            Item::Struct(item_struct) => {
                // Validate struct name doesn't contain numbers or "stream"
                let orig_name = item_struct.ident.to_string();

                // Skip trying to validate if name contains "__" as these are likely internal types
                if orig_name.contains("__") {
                    println!("  Skipping likely internal struct: {}", orig_name);
                    continue;
                }

                match validate_name(&orig_name, "Struct") {
                    Ok(_) => {
                        // Use kebab-case for struct name
                        let name = to_kebab_case(&orig_name);
                        println!("  Found struct: {} -> {}", orig_name, name);

                        let fields: Vec<String> = match &item_struct.fields {
                            syn::Fields::Named(fields) => {
                                let mut used_types = HashSet::new();
                                let mut field_strings = Vec::new();

                                for f in &fields.named {
                                    if let Some(field_ident) = &f.ident {
                                        // Validate field name doesn't contain digits
                                        let field_orig_name = field_ident.to_string();

                                        match validate_name(&field_orig_name, "Field") {
                                            Ok(_) => {
                                                // Convert field names to kebab-case
                                                let field_name = to_kebab_case(&field_orig_name);

                                                // Skip if field conversion failed
                                                if field_name.is_empty() {
                                                    println!("    Skipping field with empty name conversion");
                                                    continue;
                                                }

                                                let field_type = match rust_type_to_wit(
                                                    &f.ty,
                                                    &mut used_types,
                                                ) {
                                                    Ok(ty) => ty,
                                                    Err(e) => {
                                                        println!(
                                                            "    Error converting field type: {}",
                                                            e
                                                        );
                                                        return Err(e);
                                                    }
                                                };

                                                println!(
                                                    "    Field: {} -> {}",
                                                    field_name, field_type
                                                );
                                                field_strings.push(format!(
                                                    "        {}: {}",
                                                    field_name, field_type
                                                ));
                                            }
                                            Err(e) => {
                                                println!(
                                                    "    Skipping field with invalid name: {}",
                                                    e
                                                );
                                                continue;
                                            }
                                        }
                                    }
                                }

                                field_strings
                            }
                            _ => Vec::new(),
                        };

                        if !fields.is_empty() {
                            type_defs.insert(
                                name.clone(),
                                format!("    record {} {{\n{}\n    }}", name, fields.join(",\n")),
                            );
                        }
                    }
                    Err(e) => {
                        println!("  Skipping struct with invalid name: {}", e);
                        continue;
                    }
                }
            }
            Item::Enum(item_enum) => {
                // Validate enum name doesn't contain numbers or "stream"
                let orig_name = item_enum.ident.to_string();

                // Skip trying to validate if name contains "__" as these are likely internal types
                if orig_name.contains("__") {
                    println!("  Skipping likely internal enum: {}", orig_name);
                    continue;
                }

                match validate_name(&orig_name, "Enum") {
                    Ok(_) => {
                        // Use kebab-case for enum name
                        let name = to_kebab_case(&orig_name);
                        println!("  Found enum: {} -> {}", orig_name, name);

                        let mut variants = Vec::new();
                        let mut skip_enum = false;

                        for v in &item_enum.variants {
                            let variant_orig_name = v.ident.to_string();

                            // Validate variant name
                            match validate_name(&variant_orig_name, "Enum variant") {
                                Ok(_) => {
                                    match &v.fields {
                                        syn::Fields::Unnamed(fields)
                                            if fields.unnamed.len() == 1 =>
                                        {
                                            let mut used_types = HashSet::new();

                                            match rust_type_to_wit(
                                                &fields.unnamed.first().unwrap().ty,
                                                &mut used_types,
                                            ) {
                                                Ok(ty) => {
                                                    // Use kebab-case for variant names and use parentheses for type
                                                    let variant_name =
                                                        to_kebab_case(&variant_orig_name);
                                                    println!(
                                                        "    Variant: {} -> {}({})",
                                                        variant_orig_name, variant_name, ty
                                                    );
                                                    variants.push(format!(
                                                        "        {}({})",
                                                        variant_name, ty
                                                    ));
                                                }
                                                Err(e) => {
                                                    println!(
                                                        "    Error converting variant type: {}",
                                                        e
                                                    );
                                                    return Err(e);
                                                }
                                            }
                                        }
                                        syn::Fields::Unit => {
                                            // Use kebab-case for variant names
                                            let variant_name = to_kebab_case(&variant_orig_name);
                                            println!(
                                                "    Variant: {} -> {}",
                                                variant_orig_name, variant_name
                                            );
                                            variants.push(format!("        {}", variant_name));
                                        }
                                        _ => {
                                            println!(
                                                "    Skipping complex variant: {}",
                                                variant_orig_name
                                            );
                                            // Complex variants with multiple fields aren't directly supported in WIT
                                            // For simplicity, we'll skip enums with complex variants
                                            skip_enum = true;
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("    Skipping variant with invalid name: {}", e);
                                    skip_enum = true;
                                    break;
                                }
                            }
                        }

                        if !skip_enum && !variants.is_empty() {
                            type_defs.insert(
                                name.clone(),
                                format!(
                                    "    variant {} {{\n{}\n    }}",
                                    name,
                                    variants.join(",\n")
                                ),
                            );
                        }
                    }
                    Err(e) => {
                        println!("  Skipping enum with invalid name: {}", e);
                        continue;
                    }
                }
            }
            _ => {}
        }
    }

    println!("Collected {} type definitions from file", type_defs.len());
    Ok(type_defs)
}

// Find all relevant Rust projects
fn find_rust_projects(base_dir: &Path) -> Vec<PathBuf> {
    let mut projects = Vec::new();
    println!("Scanning for Rust projects in {}", base_dir.display());

    for entry in WalkDir::new(base_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();

        if path.is_dir() && path != base_dir {
            let cargo_toml = path.join("Cargo.toml");
            println!("Checking {}", cargo_toml.display());

            if cargo_toml.exists() {
                // Try to read and parse Cargo.toml
                if let Ok(content) = fs::read_to_string(&cargo_toml) {
                    if let Ok(cargo_data) = content.parse::<Value>() {
                        // Check for the specific metadata
                        if let Some(metadata) = cargo_data
                            .get("package")
                            .and_then(|p| p.get("metadata"))
                            .and_then(|m| m.get("component"))
                        {
                            if let Some(package) = metadata.get("package") {
                                if let Some(package_str) = package.as_str() {
                                    println!(
                                        "  Found package.metadata.component.package = {:?}",
                                        package_str
                                    );
                                    if package_str == "hyperware:process" {
                                        println!("  Adding project: {}", path.display());
                                        projects.push(path.to_path_buf());
                                    }
                                }
                            }
                        } else {
                            println!("  No package.metadata.component metadata found");
                        }
                    }
                }
            }
        }
    }

    println!("Found {} relevant Rust projects", projects.len());
    projects
}

// Helper function to generate signature struct for specific attribute type
fn generate_signature_struct(
    kebab_name: &str,
    attr_type: &str,
    method: &syn::ImplItemFn,
    used_types: &mut HashSet<String>,
) -> Result<String> {
    // Create signature struct name with attribute type
    let signature_struct_name = format!("{}-signature-{}", kebab_name, attr_type);

    // Generate comment for this specific function
    let comment = format!(
        "    // Function signature for: {} ({})",
        kebab_name, attr_type
    );

    // Create struct fields that directly represent function parameters
    let mut struct_fields = Vec::new();

    // Add target parameter based on attribute type
    if attr_type == "http" {
        struct_fields.push("        target: string".to_string());
    } else {
        // remote or local
        struct_fields.push("        target: address".to_string());
    }

    // Process function parameters (skip &self and &mut self)
    for arg in &method.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                // Skip &self and &mut self
                if pat_ident.ident == "self" {
                    continue;
                }

                // Get original param name and convert to kebab-case
                let param_orig_name = pat_ident.ident.to_string();

                // Validate parameter name
                match validate_name(&param_orig_name, "Parameter") {
                    Ok(_) => {
                        let param_name = to_kebab_case(&param_orig_name);

                        // Rust type to WIT type
                        match rust_type_to_wit(&pat_type.ty, used_types) {
                            Ok(param_type) => {
                                // Add field directly to the struct
                                struct_fields
                                    .push(format!("        {}: {}", param_name, param_type));
                            }
                            Err(e) => {
                                println!("    Error converting parameter type: {}", e);
                                return Err(e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("    Skipping parameter with invalid name: {}", e);
                        return Err(e);
                    }
                }
            }
        }
    }

    // Add return type field
    match &method.sig.output {
        syn::ReturnType::Type(_, ty) => match rust_type_to_wit(&*ty, used_types) {
            Ok(return_type) => {
                struct_fields.push(format!("        returning: {}", return_type));
            }
            Err(e) => {
                println!("    Error converting return type: {}", e);
                return Err(e);
            }
        },
        _ => {
            // For unit return type
            struct_fields.push("        returning: unit".to_string());
        }
    }

    // Combine everything into a record definition
    let record_def = format!(
        "{}\n    record {} {{\n{}\n    }}",
        comment,
        signature_struct_name,
        struct_fields.join(",\n")
    );

    Ok(record_def)
}

// Helper trait to get TypePath from Type
trait AsTypePath {
    fn as_type_path(&self) -> Option<&syn::TypePath>;
}

impl AsTypePath for syn::Type {
    fn as_type_path(&self) -> Option<&syn::TypePath> {
        match self {
            syn::Type::Path(tp) => Some(tp),
            _ => None,
        }
    }
}

// Process a single Rust project and generate WIT files
fn process_rust_project(project_path: &Path, api_dir: &Path) -> Result<Option<(String, String)>> {
    println!("\nProcessing project: {}", project_path.display());

    // Find lib.rs for this project
    let lib_rs = project_path.join("src").join("lib.rs");

    if !lib_rs.exists() {
        println!("No lib.rs found for project: {}", project_path.display());
        return Ok(None);
    }

    // Find all Rust files in the project
    let rust_files = find_rust_files(project_path);

    // Collect all type definitions from all Rust files
    let mut all_type_defs = HashMap::new();
    for file_path in &rust_files {
        match collect_type_definitions_from_file(file_path) {
            Ok(file_type_defs) => {
                for (name, def) in file_type_defs {
                    all_type_defs.insert(name, def);
                }
            }
            Err(e) => {
                println!(
                    "Error collecting type definitions from {}: {}",
                    file_path.display(),
                    e
                );
                // Continue with other files
            }
        }
    }

    println!("Collected {} total type definitions", all_type_defs.len());

    // Parse lib.rs to find the hyperprocess attribute and interface details
    let lib_content = fs::read_to_string(&lib_rs).with_context(|| {
        format!(
            "Failed to read lib.rs for project: {}",
            project_path.display()
        )
    })?;

    let ast = syn::parse_file(&lib_content).with_context(|| {
        format!(
            "Failed to parse lib.rs for project: {}",
            project_path.display()
        )
    })?;

    let mut wit_world = None;
    let mut interface_name = None;
    let mut kebab_interface_name = None;
    let mut impl_item_with_hyperprocess = None;

    println!("Scanning for impl blocks with hyperprocess attribute");
    for item in &ast.items {
        if let Item::Impl(impl_item) = item {
            // Check if this impl block has a #[hyperprocess] attribute
            if let Some(attr) = impl_item
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("hyperprocess"))
            {
                println!("Found hyperprocess attribute");

                // Extract the wit_world name
                match extract_wit_world(&[attr.clone()]) {
                    Ok(world_name) => {
                        println!("Extracted wit_world: {}", world_name);
                        wit_world = Some(world_name);

                        // Get the interface name from the impl type
                        interface_name = impl_item.self_ty.as_ref().as_type_path().map(|tp| {
                            if let Some(last_segment) = tp.path.segments.last() {
                                last_segment.ident.to_string()
                            } else {
                                "Unknown".to_string()
                            }
                        });

                        // Check for "State" suffix and remove it
                        if let Some(ref name) = interface_name {
                            // Validate the interface name
                            if let Err(e) = validate_name(name, "Interface") {
                                println!("Interface name validation failed: {}", e);
                                continue;
                            }

                            // Remove State suffix if present
                            let base_name = remove_state_suffix(name);

                            // Convert to kebab-case for file name and interface name
                            kebab_interface_name = Some(to_kebab_case(&base_name));

                            println!("Interface name: {:?}", interface_name);
                            println!("Base name: {}", base_name);
                            println!("Kebab interface name: {:?}", kebab_interface_name);

                            // Save the impl item for later processing
                            impl_item_with_hyperprocess = Some(impl_item.clone());
                        }
                    }
                    Err(e) => println!("Failed to extract wit_world: {}", e),
                }
            }
        }
    }

    // Now generate the WIT content for the interface
    if let (Some(ref iface_name), Some(ref kebab_name), Some(ref impl_item)) = (
        &interface_name,
        &kebab_interface_name,
        &impl_item_with_hyperprocess,
    ) {
        let mut signature_structs = Vec::new();
        let mut used_types = HashSet::new();

        for item in &impl_item.items {
            if let ImplItem::Fn(method) = item {
                let method_name = method.sig.ident.to_string();
                println!("  Examining method: {}", method_name);

                // Check for attribute types
                let has_remote = method
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("remote"));
                let has_local = method
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("local"));
                let has_http = method.attrs.iter().any(|attr| attr.path().is_ident("http"));

                if has_remote || has_local || has_http {
                    println!(
                        "    Has relevant attributes: remote={}, local={}, http={}",
                        has_remote, has_local, has_http
                    );

                    // Validate function name
                    match validate_name(&method_name, "Function") {
                        Ok(_) => {
                            // Convert function name to kebab-case
                            let kebab_name = to_kebab_case(&method_name);
                            println!("    Processing method: {} -> {}", method_name, kebab_name);

                            // Generate a signature struct for each attribute type
                            if has_remote {
                                match generate_signature_struct(
                                    &kebab_name,
                                    "remote",
                                    method,
                                    &mut used_types,
                                ) {
                                    Ok(remote_struct) => signature_structs.push(remote_struct),
                                    Err(e) => println!(
                                        "    Error generating remote signature struct: {}",
                                        e
                                    ),
                                }
                            }

                            if has_local {
                                match generate_signature_struct(
                                    &kebab_name,
                                    "local",
                                    method,
                                    &mut used_types,
                                ) {
                                    Ok(local_struct) => signature_structs.push(local_struct),
                                    Err(e) => println!(
                                        "    Error generating local signature struct: {}",
                                        e
                                    ),
                                }
                            }

                            if has_http {
                                match generate_signature_struct(
                                    &kebab_name,
                                    "http",
                                    method,
                                    &mut used_types,
                                ) {
                                    Ok(http_struct) => signature_structs.push(http_struct),
                                    Err(e) => println!(
                                        "    Error generating HTTP signature struct: {}",
                                        e
                                    ),
                                }
                            }
                        }
                        Err(e) => {
                            println!("    Skipping method with invalid name: {}", e);
                        }
                    }
                } else {
                    println!("    Skipping method without relevant attributes");
                }
            }
        }

        // Include all defined types, not just the ones used in interface functions
        println!("Including all defined types ({})", all_type_defs.len());

        // Convert all type definitions to a vector
        let mut type_defs: Vec<String> = all_type_defs.values().cloned().collect();

        // Sort them for consistent output
        type_defs.sort();

        // Generate the final WIT content
        if signature_structs.is_empty() {
            println!("No functions found for interface {}", iface_name);
        } else {
            // Start with the interface comment
            let mut content = "    // This interface contains function signature definitions that will be used\n    // by the hyper-bindgen macro to generate async function bindings.\n    //\n    // NOTE: This is currently a hacky workaround since WIT async functions are not\n    // available until WASI Preview 3. Once Preview 3 is integrated into Hyperware,\n    // we should switch to using proper async WIT function signatures instead of\n    // this struct-based approach with hyper-bindgen generating the async stubs.\n".to_string();

            // Add standard imports
            content.push_str("\n    use standard.{address};\n\n");

            // Add type definitions if any
            if !type_defs.is_empty() {
                content.push_str(&type_defs.join("\n\n"));
                content.push_str("\n\n");
            }

            // Add signature structs
            content.push_str(&signature_structs.join("\n\n"));

            // Wrap in interface block
            let final_content = format!("interface {} {{\n{}\n}}\n", kebab_name, content);
            println!(
                "Generated interface content for {} with {} signature structs",
                iface_name,
                signature_structs.len()
            );

            // Write the interface file with kebab-case name
            let interface_file = api_dir.join(format!("{}.wit", kebab_name));
            println!("Writing WIT file to {}", interface_file.display());

            fs::write(&interface_file, &final_content)
                .with_context(|| format!("Failed to write {}", interface_file.display()))?;

            println!("Successfully wrote WIT file");
        }
    }

    if let (Some(wit_world), Some(_), Some(kebab_iface)) =
        (wit_world, interface_name, kebab_interface_name)
    {
        println!("Returning import statement for interface {}", kebab_iface);
        // Use kebab-case interface name for import
        Ok(Some((format!("    import {};", kebab_iface), wit_world)))
    } else {
        println!("No valid interface found");
        Ok(None)
    }
}

fn rewrite_wit(
    api_dir: &Path,
    new_imports: &Vec<String>,
    wit_worlds: &mut HashSet<String>,
    updated_world: &mut bool,
) -> Result<()> {
    for entry in WalkDir::new(api_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "wit") {
            println!("Checking WIT file: {}", path.display());

            if let Ok(content) = fs::read_to_string(path) {
                if content.contains("world ") {
                    println!("Found world definition file");

                    // Extract the world name and existing imports
                    let lines: Vec<&str> = content.lines().collect();
                    let mut world_name = None;
                    let mut existing_imports = Vec::new();
                    let mut include_line = "    include process-v1;".to_string();

                    for line in &lines {
                        let trimmed = line.trim();

                        if trimmed.starts_with("world ") {
                            if let Some(name) = trimmed.split_whitespace().nth(1) {
                                world_name = Some(name.trim_end_matches(" {").to_string());
                            }
                        } else if trimmed.starts_with("import ") {
                            existing_imports.push(trimmed.to_string());
                        } else if trimmed.starts_with("include ") {
                            include_line = trimmed.to_string();
                        }
                    }

                    if let Some(world_name) = world_name {
                        println!("Extracted world name: {}", world_name);

                        // Check if this world name matches the one we're looking for
                        if wit_worlds.remove(&world_name) || wit_worlds.contains(&world_name[6..]) {
                            // Determine the include line based on world name
                            // If world name starts with "types-", use "include lib;" instead
                            if world_name.starts_with("types-") {
                                include_line = "    include lib;".to_string();
                            } else {
                                // Keep existing include or default to process-v1
                                if !include_line.contains("include ") {
                                    include_line = "    include process-v1;".to_string();
                                }
                            }

                            // Combine existing imports with new imports
                            let mut all_imports = existing_imports.clone();

                            for import in new_imports {
                                let import_stmt = import.trim();
                                if !all_imports.iter().any(|i| i.trim() == import_stmt) {
                                    all_imports.push(import_stmt.to_string());
                                }
                            }

                            // Make sure all imports have proper indentation
                            let all_imports_with_indent: Vec<String> = all_imports
                                .iter()
                                .map(|import| {
                                    if import.starts_with("    ") {
                                        import.clone()
                                    } else {
                                        format!("    {}", import.trim())
                                    }
                                })
                                .collect();

                            let imports_section = all_imports_with_indent.join("\n");

                            // Create updated world content with proper indentation
                            let world_content = format!(
                                "world {} {{\n{}\n    {}\n}}",
                                world_name,
                                imports_section,
                                include_line.trim()
                            );

                            println!("Writing updated world definition to {}", path.display());
                            // Write the updated world file
                            fs::write(path, world_content).with_context(|| {
                                format!("Failed to write updated world file: {}", path.display())
                            })?;

                            println!("Successfully updated world definition");
                            *updated_world = true;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

// Generate WIT files from Rust code
pub fn generate_wit_files(
    base_dir: &Path,
    api_dir: &Path,
    is_recursive_call: bool,
) -> Result<(Vec<PathBuf>, Vec<String>)> {
    fs::create_dir_all(&api_dir)?;

    // Find all relevant Rust projects
    let projects = find_rust_projects(base_dir);
    let mut processed_projects = Vec::new();

    if projects.is_empty() {
        println!("No relevant Rust projects found.");
        return Ok((Vec::new(), Vec::new()));
    }

    // Process each project and collect world imports
    let mut new_imports = Vec::new();
    let mut interfaces = Vec::new();

    let mut wit_worlds = HashSet::new();
    for project_path in &projects {
        println!("Processing project: {}", project_path.display());

        match process_rust_project(project_path, api_dir) {
            Ok(Some((import, wit_world))) => {
                println!("Got import statement: {}", import);
                new_imports.push(import.clone());

                // Extract interface name from import statement
                let interface_name = import
                    .trim_start_matches("    import ")
                    .trim_end_matches(";")
                    .to_string();

                interfaces.push(interface_name);
                processed_projects.push(project_path.clone());

                wit_worlds.insert(wit_world);
            }
            Ok(None) => println!("No import statement generated"),
            Err(e) => println!("Error processing project: {}", e),
        }
    }

    println!("Collected {} new imports", new_imports.len());

    // Check for existing world definition files and update them
    println!("Looking for existing world definition files");
    let mut updated_world = false;

    rewrite_wit(api_dir, &new_imports, &mut wit_worlds, &mut updated_world)?;

    let rerun_rewrite_wit = !wit_worlds.is_empty();
    for wit_world in wit_worlds {
        // Create a new file with the simple world definition
        let new_file_path = api_dir.join(format!("{}.wit", wit_world));
        let simple_world_content = format!("world {} {{}}", wit_world);

        println!(
            "Creating new world definition file: {}",
            new_file_path.display()
        );
        fs::write(&new_file_path, simple_world_content).with_context(|| {
            format!(
                "Failed to create new world file: {}",
                new_file_path.display()
            )
        })?;

        let new_file_path = api_dir.join(format!("types-{}.wit", wit_world));
        let simple_world_content = format!("world types-{} {{}}", wit_world);

        println!(
            "Creating new world definition file: {}",
            new_file_path.display()
        );
        fs::write(&new_file_path, simple_world_content).with_context(|| {
            format!(
                "Failed to create new world file: {}",
                new_file_path.display()
            )
        })?;

        println!("Successfully created new world definition file");
        updated_world = true;
    }

    if rerun_rewrite_wit && !is_recursive_call {
        return generate_wit_files(base_dir, api_dir, true);
    }

    // If no world definitions were found, create a default one
    if !updated_world && !new_imports.is_empty() {
        // Define default world name
        let default_world = "async-app-template-dot-os-v0";
        println!(
            "No existing world definitions found, creating default with name: {}",
            default_world
        );

        // Create world content with process-v1 include and proper indentation for imports
        let imports_with_indent: Vec<String> = new_imports
            .iter()
            .map(|import| {
                if import.starts_with("    ") {
                    import.clone()
                } else {
                    format!("    {}", import.trim())
                }
            })
            .collect();

        // Determine include based on world name
        let include_line = if default_world.starts_with("types-") {
            "include lib;"
        } else {
            "include process-v1;"
        };

        let world_content = format!(
            "world {} {{\n{}\n    {}\n}}",
            default_world,
            imports_with_indent.join("\n"),
            include_line
        );

        let world_file = api_dir.join(format!("{}.wit", default_world));
        println!(
            "Writing default world definition to {}",
            world_file.display()
        );

        fs::write(&world_file, world_content).with_context(|| {
            format!(
                "Failed to write default world file: {}",
                world_file.display()
            )
        })?;

        println!("Successfully created default world definition");
    }

    println!("WIT files generated successfully in the 'api' directory.");
    Ok((processed_projects, interfaces))
}
