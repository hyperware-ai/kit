use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use color_eyre::{
    eyre::{bail, eyre, WrapErr},
    Result,
};
use syn::{self, Attribute, ImplItem, Item, Type};
use toml::Value;
use tracing::{debug, info, instrument, warn};
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
#[instrument(level = "trace", skip_all)]
fn extract_wit_world(attrs: &[Attribute]) -> Result<String> {
    for attr in attrs {
        if attr.path().is_ident("hyperprocess") {
            // Convert attribute to string representation
            let attr_str = format!("{:?}", attr);
            debug!("Attribute string: {}", attr_str);

            // Look for wit_world in the attribute string
            if let Some(pos) = attr_str.find("wit_world") {
                debug!("Found wit_world at position {}", pos);

                // Find the literal value after wit_world by looking for lit: "value"
                let lit_pattern = "lit: \"";
                if let Some(lit_pos) = attr_str[pos..].find(lit_pattern) {
                    let start_pos = pos + lit_pos + lit_pattern.len();

                    // Find the closing quote of the literal
                    if let Some(quote_pos) = attr_str[start_pos..].find('\"') {
                        let world_name = &attr_str[start_pos..(start_pos + quote_pos)];
                        debug!("Extracted wit_world: {}", world_name);
                        return Ok(world_name.to_string());
                    }
                }
            }
        }
    }
    bail!("wit_world not found in hyperprocess attribute")
}

// Convert Rust type to WIT type, including downstream types
#[instrument(level = "trace", skip_all)]
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
#[instrument(level = "trace", skip_all)]
fn find_rust_files(crate_path: &Path) -> Vec<PathBuf> {
    let mut rust_files = Vec::new();
    let src_dir = crate_path.join("src");

    debug!("Finding Rust files in {}", src_dir.display());

    if !src_dir.exists() || !src_dir.is_dir() {
        warn!("No src directory found at {}", src_dir.display());
        return rust_files;
    }

    for entry in WalkDir::new(src_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
            debug!("Found Rust file: {}", path.display());
            rust_files.push(path.to_path_buf());
        }
    }

    debug!("Found {} Rust files", rust_files.len());
    rust_files
}

// Sanitize field names by removing leading underscores
fn sanitize_field_name(field_name: String) -> String {
    if let Some(stripped) = field_name.strip_prefix('_') {
        println!(
            "    Warning: Field '{}' starts with underscore, removing it",
            field_name
        );
        stripped.to_string()
    } else {
        field_name
    }
}

// Collect **only used** type definitions (structs and enums) from a file
#[instrument(level = "trace", skip_all)]
fn collect_type_definitions_from_file(
    file_path: &Path,
    used_types: &HashSet<String>, // Accept the set of used types
) -> Result<HashMap<String, String>> {
    debug!(
        "Collecting used type definitions from file: {}",
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
                    // This skip can remain, as internal types are unlikely to be in `used_types` anyway
                    warn!("Skipping likely internal struct: {}", orig_name);
                    continue;
                }

                match validate_name(&orig_name, "Struct") {
                    Ok(_) => {
                        // Use kebab-case for struct name
                        let name = to_kebab_case(&orig_name);

                        // --- Check if this type is used ---
                        if !used_types.contains(&name) {
                            // Skip this struct if not in the used set
                            continue;
                        }
                        // --- End Check ---

                        debug!("  Found used struct: {} -> {}", orig_name, name);

                        // Proceed with field processing only if the struct is used
                        let fields: Vec<String> = match &item_struct.fields {
                            syn::Fields::Named(fields) => {
                                // Note: The `rust_type_to_wit` calls here still use a *local* `used_types`
                                // set for *recursive* type discovery *within this struct's definition*.
                                // This is necessary for correctly formatting types like list<other-used-type>.
                                // The main `used_types` set (passed as argument) determines *if* this struct
                                // definition is included at all.
                                let mut local_used_types_for_fields = HashSet::new(); // Renamed for clarity
                                let mut field_strings = Vec::new();

                                for f in &fields.named {
                                    if let Some(field_ident) = &f.ident {
                                        let field_orig_name = field_ident.to_string();
                                        match validate_name(&field_orig_name, "Field") {
                                            Ok(_) => {
                                                let field_name = to_kebab_case(&field_orig_name);
                                                if field_name.is_empty() {
                                                    warn!(
                                                        "Skipping field with empty name conversion"
                                                    );
                                                    continue;
                                                }

                                                // This call populates `local_used_types_for_fields` if needed,
                                                // but its primary goal here is WIT type string generation.
                                                let field_type = match rust_type_to_wit(
                                                    &f.ty,
                                                    &mut local_used_types_for_fields, // Pass the local set
                                                ) {
                                                    Ok(ty) => ty,
                                                    Err(e) => {
                                                        warn!("Error converting field type: {}", e);
                                                        // Propagate error if field type conversion fails
                                                        return Err(e);
                                                    }
                                                };

                                                debug!(
                                                    "    Field: {} -> {}",
                                                    field_name, field_type
                                                );
                                                field_strings.push(format!(
                                                    "        {}: {}",
                                                    field_name, field_type
                                                ));
                                            }
                                            Err(e) => {
                                                warn!(
                                                    "    Skipping field with invalid name: {}",
                                                    e
                                                );
                                                // Decide if you want to continue or error out
                                                continue;
                                            }
                                        }
                                    }
                                }
                                field_strings
                            }
                            _ => Vec::new(), // Handle tuple structs, unit structs if needed
                        };

                        // Add the struct definition only if it has fields (or adjust logic if empty records are valid)
                        if !fields.is_empty() {
                            type_defs.insert(
                                name.clone(),
                                format!("    record {} {{\n{}\n    }}", name, fields.join(",\n")),
                            );
                        } else {
                            warn!("  Skipping used struct {} with no convertible fields", name);
                        }
                    }
                    Err(e) => {
                        // Struct name validation failed, skip regardless of usage
                        warn!("  Skipping struct with invalid name: {}", e);
                        continue;
                    }
                }
            }
            Item::Enum(item_enum) => {
                // Validate enum name doesn't contain numbers or "stream"
                let orig_name = item_enum.ident.to_string();

                // Skip trying to validate if name contains "__"
                if orig_name.contains("__") {
                    warn!("  Skipping likely internal enum: {}", orig_name);
                    continue;
                }

                match validate_name(&orig_name, "Enum") {
                    Ok(_) => {
                        // Use kebab-case for enum name
                        let name = to_kebab_case(&orig_name);

                        // --- Check if this type is used ---
                        if !used_types.contains(&name) {
                            warn!(
                                "  Skipping type not present in any function signature: {} -> {}",
                                orig_name, name
                            ); // Optional debug log
                            continue; // Skip this enum if not in the used set
                        }
                        // --- End Check ---

                        debug!("  Found used enum: {} -> {}", orig_name, name);

                        // Proceed with variant processing only if the enum is used
                        let mut variants = Vec::new();
                        let mut skip_enum = false;

                        for v in &item_enum.variants {
                            let variant_orig_name = v.ident.to_string();
                            match validate_name(&variant_orig_name, "Enum variant") {
                                Ok(_) => {
                                    match &v.fields {
                                        syn::Fields::Unnamed(fields)
                                            if fields.unnamed.len() == 1 =>
                                        {
                                            // Similar to structs, use a local set for inner type resolution
                                            let mut local_used_types_for_variant = HashSet::new();
                                            match rust_type_to_wit(
                                                &fields.unnamed.first().unwrap().ty,
                                                &mut local_used_types_for_variant, // Pass local set
                                            ) {
                                                Ok(ty) => {
                                                    let variant_name =
                                                        to_kebab_case(&variant_orig_name);
                                                    debug!(
                                                        "    Variant: {} -> {}({})",
                                                        variant_orig_name, variant_name, ty
                                                    );
                                                    variants.push(format!(
                                                        "        {}({})",
                                                        variant_name, ty
                                                    ));
                                                }
                                                Err(e) => {
                                                    warn!(
                                                        "    Error converting variant type: {}",
                                                        e
                                                    );
                                                    // Propagate error if variant type conversion fails
                                                    return Err(e);
                                                }
                                            }
                                        }
                                        syn::Fields::Unit => {
                                            let variant_name = to_kebab_case(&variant_orig_name);
                                            debug!(
                                                "    Variant: {} -> {}",
                                                variant_orig_name, variant_name
                                            );
                                            variants.push(format!("        {}", variant_name));
                                        }
                                        _ => {
                                            warn!(
                                                "    Skipping complex variant in used enum {}: {}",
                                                name, variant_orig_name
                                            );
                                            skip_enum = true; // Skip the whole enum if one variant is complex
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("    Skipping variant with invalid name in used enum {}: {}", name, e);
                                    skip_enum = true; // Skip the whole enum if one variant name is invalid
                                    break;
                                }
                            }
                        }

                        // Add the enum definition only if it wasn't skipped and has variants
                        if !skip_enum && !variants.is_empty() {
                            type_defs.insert(
                                name.clone(),
                                format!(
                                    "    variant {} {{\n{}\n    }}",
                                    name,
                                    variants.join(",\n")
                                ),
                            );
                        } else {
                            warn!("  Skipping used enum {} due to complex/invalid variants or no variants", name);
                        }
                    }
                    Err(e) => {
                        // Enum name validation failed, skip regardless of usage
                        warn!("  Skipping enum with invalid name: {}", e);
                        continue;
                    }
                }
            }
            _ => {} // Handle other top-level items like functions, impls, etc. if needed
        }
    }

    debug!(
        "Collected {} used type definitions from this file",
        type_defs.len()
    );
    Ok(type_defs)
}

// Find all relevant Rust projects
#[instrument(level = "trace", skip_all)]
fn find_rust_projects(base_dir: &Path) -> Vec<PathBuf> {
    let mut projects = Vec::new();
    debug!("Scanning for Rust projects in {}", base_dir.display());

    for entry in WalkDir::new(base_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();

        if !path.is_dir() || path == base_dir {
            continue;
        }
        let cargo_toml = path.join("Cargo.toml");
        debug!("Checking {}", cargo_toml.display());

        if !cargo_toml.exists() {
            continue;
        }
        // Try to read and parse Cargo.toml
        let Ok(content) = fs::read_to_string(&cargo_toml) else {
            continue;
        };
        let Ok(cargo_data) = content.parse::<Value>() else {
            continue;
        };
        // Check for the specific metadata
        let Some(metadata) = cargo_data
            .get("package")
            .and_then(|p| p.get("metadata"))
            .and_then(|m| m.get("component"))
        else {
            warn!(
                "  No package.metadata.component metadata found in {}",
                cargo_toml.display()
            ); // Added path context
            continue;
        };
        let Some(package) = metadata.get("package") else {
            continue;
        };
        let Some(package_str) = package.as_str() else {
            continue;
        };
        debug!(
            "  Found package.metadata.component.package = {:?}",
            package_str
        );
        if package_str == "hyperware:process" {
            debug!("  Adding project: {}", path.display()); // INFO -> DEBUG
            projects.push(path.to_path_buf());
        }
    }

    debug!("Found {} relevant Rust projects", projects.len());
    projects
}

// Helper function to generate signature struct for specific attribute type
#[instrument(level = "trace", skip_all)]
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
                        let param_name = sanitize_field_name(param_orig_name);
                        let param_name = to_kebab_case(&param_name);

                        // Rust type to WIT type
                        match rust_type_to_wit(&pat_type.ty, used_types) {
                            Ok(param_type) => {
                                // Add field directly to the struct
                                struct_fields
                                    .push(format!("        {}: {}", param_name, param_type));
                            }
                            Err(e) => {
                                warn!("    Error converting parameter type: {}", e);
                                return Err(e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("    Skipping parameter with invalid name: {}", e);
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
                warn!("    Error converting return type: {}", e);
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
#[instrument(level = "trace", skip_all)]
fn process_rust_project(project_path: &Path, api_dir: &Path) -> Result<Option<(String, String)>> {
    info!("Processing project: {}", project_path.display());

    // Find lib.rs for this project
    let lib_rs = project_path.join("src").join("lib.rs");

    if !lib_rs.exists() {
        warn!("No lib.rs found for project: {}", project_path.display());
        return Ok(None);
    }

    // Find all Rust files in the project
    let rust_files = find_rust_files(project_path);

    // Parse lib.rs to find the hyperprocess attribute and interface details first
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

    debug!("Scanning for impl blocks with hyperprocess attribute");
    for item in &ast.items {
        let Item::Impl(impl_item) = item else {
            continue;
        };
        // Check if this impl block has a #[hyperprocess] attribute
        if let Some(attr) = impl_item
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("hyperprocess"))
        {
            debug!("Found hyperprocess attribute");

            // Extract the wit_world name
            match extract_wit_world(&[attr.clone()]) {
                Ok(world_name) => {
                    debug!("Extracted wit_world: {}", world_name);
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
                    let Some(ref name) = interface_name else {
                        continue;
                    };
                    // Validate the interface name
                    if let Err(e) = validate_name(name, "Interface") {
                        warn!("Interface name validation failed: {}", e);
                        continue; // Skip this impl block if validation fails
                    }

                    // Remove State suffix if present
                    let base_name = remove_state_suffix(name);

                    // Convert to kebab-case for file name and interface name
                    kebab_interface_name = Some(to_kebab_case(&base_name));

                    debug!("Interface name: {:?}", interface_name);
                    debug!("Base name: {}", base_name);
                    debug!("Kebab interface name: {:?}", kebab_interface_name);

                    // Save the impl item for later processing
                    impl_item_with_hyperprocess = Some(impl_item.clone());
                    break; // Assume only one hyperprocess impl block per lib.rs
                }
                Err(e) => warn!("Failed to extract wit_world: {}", e),
            }
        }
    }

    // Prepare to collect signature structs and used types
    let mut signature_structs = Vec::new();
    let mut used_types = HashSet::new(); // This will be populated now

    // Analyze the functions within the identified impl block (if found)
    if let Some(ref impl_item) = &impl_item_with_hyperprocess {
        if let Some(ref _kebab_name) = &kebab_interface_name {
            // Ensure kebab_name is available but acknowledge unused in this block
            for item in &impl_item.items {
                let ImplItem::Fn(method) = item else {
                    continue;
                };
                let method_name = method.sig.ident.to_string();
                debug!("Examining method: {}", method_name);

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
                    debug!(
                        "    Has relevant attributes: remote={}, local={}, http={}",
                        has_remote, has_local, has_http
                    );

                    // Validate function name
                    match validate_name(&method_name, "Function") {
                        Ok(_) => {
                            // Convert function name to kebab-case
                            let func_kebab_name = to_kebab_case(&method_name); // Use different var name
                            debug!(
                                "    Processing method: {} -> {}",
                                method_name, func_kebab_name
                            );

                            // Generate a signature struct for each attribute type
                            // This will populate `used_types`
                            if has_remote {
                                match generate_signature_struct(
                                    &func_kebab_name, // Pass func kebab name
                                    "remote",
                                    method,
                                    &mut used_types, // Pass the main set
                                ) {
                                    Ok(remote_struct) => signature_structs.push(remote_struct),
                                    Err(e) => {
                                        warn!("    Error generating remote signature struct: {}", e)
                                    }
                                }
                            }

                            if has_local {
                                match generate_signature_struct(
                                    &func_kebab_name, // Pass func kebab name
                                    "local",
                                    method,
                                    &mut used_types, // Pass the main set
                                ) {
                                    Ok(local_struct) => signature_structs.push(local_struct),
                                    Err(e) => {
                                        warn!("    Error generating local signature struct: {}", e)
                                    }
                                }
                            }

                            if has_http {
                                match generate_signature_struct(
                                    &func_kebab_name, // Pass func kebab name
                                    "http",
                                    method,
                                    &mut used_types, // Pass the main set
                                ) {
                                    Ok(http_struct) => signature_structs.push(http_struct),
                                    Err(e) => {
                                        warn!("    Error generating HTTP signature struct: {}", e)
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("    Skipping method with invalid name: {}", e);
                        }
                    }
                } else {
                    warn!("    Method {} does not have the [remote], [local], or [http], and will not be included in the WIT file", method_name);
                }
            }
        }
    }
    debug!(
        "Identified {} used types from function signatures.",
        used_types.len()
    );

    // Collect **only used** type definitions from all Rust files
    let mut all_type_defs = HashMap::new(); // Now starts empty, filled by collector
    for file_path in &rust_files {
        // Pass the populated used_types set to the collector
        match collect_type_definitions_from_file(file_path, &used_types) {
            Ok(file_type_defs) => {
                for (name, def) in file_type_defs {
                    // Since the collector only returns used types, we can insert directly
                    all_type_defs.insert(name, def);
                }
            }
            Err(e) => {
                warn!(
                    "Error collecting type definitions from {}: {}",
                    file_path.display(),
                    e
                );
                // Continue with other files
            }
        }
    }

    debug!("Collected {} used type definitions", all_type_defs.len());

    // Now generate the WIT content for the interface
    if let (Some(ref iface_name), Some(ref kebab_name), Some(ref _impl_item)) = (
        // impl_item no longer needed here
        &interface_name,
        &kebab_interface_name,
        &impl_item_with_hyperprocess, // Keep this condition to ensure an interface was found
    ) {
        // No need to filter anymore, all_type_defs contains only used types
        let mut type_defs: Vec<String> = all_type_defs.into_values().collect(); // Collect values directly

        type_defs.sort(); // Sort for consistent output
        debug!("Including {} used type definitions", type_defs.len());

        // Generate the final WIT content
        if signature_structs.is_empty() && type_defs.is_empty() {
            // Check both sigs and types
            warn!(
                "No functions or used types found for interface {}",
                iface_name
            );
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

            // Add signature structs if any (moved after types for potentially better readability)
            if !signature_structs.is_empty() {
                content.push_str(&signature_structs.join("\n\n"));
            }

            // Wrap in interface block
            let final_content =
                format!("interface {} {{\n{}\n}}\n", kebab_name, content.trim_end()); // Trim trailing whitespace
            debug!(
                "Generated interface content for {} with {} signature structs and {} type definitions",
                iface_name,
                signature_structs.len(),
                type_defs.len() // Use the count from the final vector
            );

            // Write the interface file with kebab-case name
            let interface_file = api_dir.join(format!("{}.wit", kebab_name));
            debug!("Writing WIT file to {}", interface_file.display());

            fs::write(&interface_file, &final_content)
                .with_context(|| format!("Failed to write {}", interface_file.display()))?;

            debug!("Successfully wrote WIT file");
        }
    } else {
        warn!("No valid hyperprocess interface found in lib.rs");
    }

    // Return statement remains the same logic
    if let (Some(wit_world), Some(_), Some(kebab_iface)) =
        (wit_world, interface_name, kebab_interface_name)
    {
        debug!("Returning import statement for interface {}", kebab_iface);
        // Use kebab-case interface name for import
        Ok(Some((kebab_iface, wit_world)))
    } else {
        warn!("No valid interface found or wit_world extracted."); // Updated message
        Ok(None)
    }
}
#[instrument(level = "trace", skip_all)]
fn rewrite_wit(
    api_dir: &Path,
    new_imports: &Vec<String>,
    wit_worlds: &mut HashSet<String>,
    updated_world: &mut bool,
) -> Result<()> {
    debug!("Rewriting WIT world files in {}", api_dir.display());
    // handle existing api files
    for entry in WalkDir::new(api_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "wit") {
            debug!("Checking WIT file: {}", path.display());

            let Ok(content) = fs::read_to_string(path) else {
                continue;
            };
            if !content.contains("world ") {
                continue;
            }
            debug!("Found world definition file");

            // Extract the world name and existing imports
            let lines: Vec<&str> = content.lines().collect();
            let mut world_name = None;
            let mut existing_imports = Vec::new();
            let mut include_lines = HashSet::new();

            for line in &lines {
                let trimmed = line.trim();

                if trimmed.starts_with("world ") {
                    if let Some(name) = trimmed.split_whitespace().nth(1) {
                        world_name = Some(name.trim_end_matches(" {").to_string());
                    }
                } else if trimmed.starts_with("import ") {
                    existing_imports.push(trimmed.to_string());
                } else if trimmed.starts_with("include ") {
                    include_lines.insert(trimmed.to_string());
                }
            }

            let Some(world_name) = world_name else {
                continue;
            };

            debug!("Extracted world name: {}", world_name);

            // Check if this world name matches the one we're looking for
            if wit_worlds.remove(&world_name) || wit_worlds.contains(&world_name[6..]) {
                let world_content = generate_wit_file(
                    &world_name,
                    new_imports,
                    &existing_imports,
                    &mut include_lines,
                )?;

                debug!("Writing updated world definition to {}", path.display());
                // Write the updated world file
                fs::write(path, world_content).with_context(|| {
                    format!("Failed to write updated world file: {}", path.display())
                })?;

                debug!("Successfully updated world definition"); // INFO -> DEBUG
                *updated_world = true;
            }
        }
    }

    // handle non-existing api files
    for wit_world in wit_worlds.iter() {
        for prefix in ["", "types-"] {
            let wit_world = format!("{prefix}{wit_world}");
            let world_content =
                generate_wit_file(&wit_world, new_imports, &Vec::new(), &mut HashSet::new())?;

            let path = api_dir.join(format!("{wit_world}.wit"));
            debug!("Writing updated world definition to {}", path.display());
            // Write the updated world file
            fs::write(&path, world_content).with_context(|| {
                format!("Failed to write updated world file: {}", path.display())
            })?;

            debug!("Successfully created new world definition for {wit_world}");
        }
        *updated_world = true;
    }

    Ok(())
}

fn generate_wit_file(
    world_name: &str,
    new_imports: &Vec<String>,
    existing_imports: &Vec<String>,
    include_lines: &mut HashSet<String>,
) -> Result<String> {
    // Determine the include line based on world name
    // If world name starts with "types-", use "include lib;" instead
    if world_name.starts_with("types-") {
        if !include_lines.contains("include lib;") {
            include_lines.insert("include lib;".to_string());
        }
    } else {
        // Keep existing include or default to process-v1
        if include_lines.is_empty() {
            include_lines.insert("include process-v1;".to_string());
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
    let include_lines: String = include_lines.iter().map(|l| format!("    {l}\n")).collect();
    let world_content = format!("world {world_name} {{\n{imports_section}\n{include_lines}}}");

    return Ok(world_content);
}

// Generate WIT files from Rust code
#[instrument(level = "trace", skip_all)]
pub fn generate_wit_files(base_dir: &Path, api_dir: &Path) -> Result<(Vec<PathBuf>, Vec<String>)> {
    // Keep INFO for start
    info!("Generating WIT files...");
    fs::create_dir_all(&api_dir)?;

    // Find all relevant Rust projects
    let projects = find_rust_projects(base_dir);
    let mut processed_projects = Vec::new();

    if projects.is_empty() {
        warn!("No relevant Rust projects found.");
        return Ok((Vec::new(), Vec::new()));
    }

    // Process each project and collect world imports
    let mut new_imports = Vec::new();
    let mut interfaces = Vec::new();

    let mut wit_worlds = HashSet::new();
    for project_path in &projects {
        match process_rust_project(project_path, api_dir) {
            Ok(Some((interface, wit_world))) => {
                new_imports.push(format!("    import {interface};"));

                interfaces.push(interface);
                processed_projects.push(project_path.clone());

                wit_worlds.insert(wit_world);
            }
            Ok(None) => warn!(
                "No import statement generated for project: {}",
                project_path.display()
            ), // Add path context
            Err(e) => warn!("Error processing project {}: {}", project_path.display(), e), // Add path context
        }
    }

    debug!("Collected {} new imports", new_imports.len());

    // Check for existing world definition files and update them
    debug!("Looking for existing world definition files");
    let mut updated_world = false;

    rewrite_wit(api_dir, &new_imports, &mut wit_worlds, &mut updated_world)?;

    // If no world definitions were found, create a default one
    if !updated_world && !new_imports.is_empty() {
        // Define default world name
        let default_world = "async-app-template-dot-os-v0";
        warn!(
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
        debug!(
            "Writing default world definition to {}",
            world_file.display()
        );

        fs::write(&world_file, world_content).with_context(|| {
            format!(
                "Failed to write default world file: {}",
                world_file.display()
            )
        })?;

        debug!("Successfully created default world definition");
    }

    info!("WIT files generated successfully in the 'api' directory.");
    Ok((processed_projects, interfaces))
}
