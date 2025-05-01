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

// Check if a field name starts with an underscore, and if so, strip it and print a warning.
fn check_and_strip_leading_underscore(field_name: String) -> String {
    if let Some(stripped) = field_name.strip_prefix('_') {
        warn!(field_name = %field_name,
         "field_name is prefixed with an underscore, which is not allowed in WIT. Function signatures should not include unused parameters."
        );
        stripped.to_string()
    } else {
        field_name
    }
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
            debug!(attr_str = %attr_str, "Attribute string");

            // Look for wit_world in the attribute string
            if let Some(pos) = attr_str.find("wit_world") {
                debug!(pos = %pos, "Found wit_world");

                // Find the literal value after wit_world by looking for lit: "value"
                let lit_pattern = "lit: \"";
                if let Some(lit_pos) = attr_str[pos..].find(lit_pattern) {
                    let start_pos = pos + lit_pos + lit_pattern.len();

                    // Find the closing quote of the literal
                    if let Some(quote_pos) = attr_str[start_pos..].find('\"') {
                        let world_name = &attr_str[start_pos..(start_pos + quote_pos)];
                        debug!(wit_world = %world_name, "Extracted wit_world");
                        return Ok(world_name.to_string());
                    }
                }
            }
        }
    }
    bail!("wit_world not found in hyperprocess attribute")
}
// Helper function to check if a WIT type name is a primitive or known built-in
fn is_wit_primitive_or_builtin(type_name: &str) -> bool {
    matches!(
        type_name,
        "s8" | "u8"
            | "s16"
            | "u16"
            | "s32"
            | "u32"
            | "s64"
            | "u64"
            | "f32"
            | "f64"
            | "bool"
            | "char"
            | "string"
            | "address"
    ) || type_name.starts_with("list<")
        || type_name.starts_with("option<")
        || type_name.starts_with("result<")
        || type_name.starts_with("tuple<")
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
                        if args.args.len() >= 1 {
                            // Allow one or two args for Result
                            if let Some(syn::GenericArgument::Type(ok_ty)) = args.args.first() {
                                let ok_type_str = rust_type_to_wit(ok_ty, used_types)?;

                                let err_type_str = if args.args.len() >= 2 {
                                    if let Some(syn::GenericArgument::Type(err_ty)) =
                                        args.args.get(1)
                                    {
                                        rust_type_to_wit(err_ty, used_types)?
                                    } else {
                                        // Should ideally not happen if len >= 2, but handle defensively
                                        return Err(eyre!(
                                            "Failed to parse Result second generic argument"
                                        ));
                                    }
                                } else {
                                    // Only one type arg provided (e.g., Rust Result<T>)
                                    // Assume error type is empty tuple ()
                                    "tuple<>".to_string()
                                };

                                let final_ok = if ok_type_str == "tuple<>" {
                                    "_"
                                } else {
                                    &ok_type_str
                                };
                                let final_err = if err_type_str == "tuple<>" {
                                    "_"
                                } else {
                                    &err_type_str
                                };

                                let result_string = match (final_ok, final_err) {
                                    ("_", "_") => "result".to_string(),          // Shorthand: result
                                    (ok, "_") => format!("result<{}>", ok), // Shorthand: result<T>
                                    ("_", err) => format!("result<_, {}>", err), // Explicit: result<_, E>
                                    (ok, err) => format!("result<{}, {}>", ok, err), // Explicit: result<T, E>
                                };
                                Ok(result_string)
                            } else {
                                Err(eyre!("Failed to parse Result first generic argument"))
                            }
                        } else {
                            Err(eyre!("Result requires at least one type argument in Rust"))
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
                debug!("Empty tuple is tuple<> in WIT");
                // Empty tuple is tuple<> in WIT
                Ok("tuple<>".to_string())
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

    debug!(src_dir = %src_dir.display(), "Finding Rust files");

    if !src_dir.exists() || !src_dir.is_dir() {
        warn!(src_dir = %src_dir.display(), "No src directory found");
        return rust_files;
    }

    for entry in WalkDir::new(src_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "rs") {
            debug!(path = %path.display(), "Found Rust file");
            rust_files.push(path.to_path_buf());
        }
    }

    debug!(count = %rust_files.len(), "Found Rust files");
    rust_files
}

// Collect **only used** type definitions (structs and enums) from a file
#[instrument(level = "trace", skip_all)]
fn collect_type_definitions_from_file(
    file_path: &Path,
    used_types: &mut HashSet<String>, // Change to mutable, we will add to it
) -> Result<HashMap<String, String>> {
    // Keep the return type, we still build the definitions here
    debug!(
        file_path = %file_path.display(),
        "Collecting used type definitions from file"
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
                    warn!(name = %orig_name, "Skipping likely internal struct");
                    continue;
                }

                match validate_name(&orig_name, "Struct") {
                    Ok(_) => {
                        // Use kebab-case for struct name
                        let name = to_kebab_case(&orig_name);

                        // --- Check if this type is used ---
                        // NOTE: This check remains important. We only generate definitions
                        // for types that are *initially* marked as used by a function signature.
                        // However, the recursive calls below will now add *their* dependencies
                        // to the main `used_types` set for the final verification step.
                        if !used_types.contains(&name) {
                            continue;
                        }
                        // --- End Check ---

                        debug!(original_name = %orig_name, kebab_name = %name, "Found used struct");

                        let fields: Result<Vec<String>> = match &item_struct.fields {
                            // Change to collect Result
                            syn::Fields::Named(fields) => {
                                // The recursive calls to `rust_type_to_wit` below use the main `used_types`
                                // set (passed mutably to this function). This ensures that any nested custom
                                // types encountered within fields (e.g., the `T` in `list<T>`) are added to
                                // the main set for the final verification step in `process_rust_project`.
                                // The initial check `if !used_types.contains(&name)` still determines
                                // if this specific struct's definition is generated based on direct usage
                                // in function signatures.
                                let mut field_strings = Vec::new();

                                for f in &fields.named {
                                    if let Some(field_ident) = &f.ident {
                                        let field_orig_name = field_ident.to_string();
                                        match validate_name(&field_orig_name, "Field") {
                                            Ok(_) => {
                                                let field_name = to_kebab_case(&field_orig_name);
                                                if field_name.is_empty() {
                                                    warn!(
                                                        struct_name = %name, field_original_name = %field_orig_name,
                                                        "Skipping field with empty name conversion"
                                                    );
                                                    continue;
                                                }

                                                // Pass the main `used_types` set here
                                                let field_type = match rust_type_to_wit(
                                                    &f.ty, used_types, // Pass the main set
                                                ) {
                                                    Ok(ty) => ty,
                                                    Err(e) => {
                                                        // Propagate error immediately
                                                        return Err(e.wrap_err(format!("Failed to convert field '{}' in struct '{}'", field_name, name)));
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
                                                // Propagate the error instead of just warning and continuing
                                                return Err(e.wrap_err(format!(
                                                    "Invalid field name '{}' found in struct '{}'",
                                                    field_orig_name, name
                                                )));
                                            }
                                        }
                                    }
                                }
                                Ok(field_strings) // Wrap in Ok
                            }
                            _ => Ok(Vec::new()), // Handle tuple structs, unit structs if needed
                        };

                        match fields {
                            Ok(fields_vec) => {
                                if !fields_vec.is_empty() {
                                    type_defs.insert(
                                        name.clone(),
                                        format!(
                                            "    record {} {{\n{}\n    }}",
                                            name,
                                            fields_vec.join(",\n")
                                        ),
                                    );
                                } else {
                                    warn!(name = %name, "Skipping used struct with no convertible fields");
                                }
                            }
                            Err(e) => return Err(e), // Propagate error from field processing
                        }
                    }
                    Err(e) => {
                        // Return the error instead of just warning
                        return Err(
                            e.wrap_err(format!("Invalid struct name '{}' found", orig_name))
                        );
                    }
                }
            }
            Item::Enum(item_enum) => {
                // Validate enum name doesn't contain numbers or "stream"
                let orig_name = item_enum.ident.to_string();

                // Skip trying to validate if name contains "__"
                if orig_name.contains("__") {
                    debug!(name = %orig_name, "Skipping likely internal enum");
                    continue;
                }

                match validate_name(&orig_name, "Enum") {
                    Ok(_) => {
                        // Use kebab-case for enum name
                        let name = to_kebab_case(&orig_name);

                        // --- Check if this type is used ---
                        if !used_types.contains(&name) {
                            debug!(original_name = %orig_name, kebab_name = %name, "Skipping type not present in any function signature");
                            continue; // Skip this enum if not in the used set
                        }
                        debug!(original_name = %orig_name, kebab_name = %name, "Found used enum");

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
                                            // Pass the main `used_types` set here
                                            match rust_type_to_wit(
                                                &fields.unnamed.first().unwrap().ty,
                                                used_types, // Pass main set
                                            ) {
                                                Ok(ty) => {
                                                    let variant_name =
                                                        to_kebab_case(&variant_orig_name);
                                                    debug!(original_name = %variant_orig_name, kebab_name = %variant_name, ty_str = %ty, "Found enum variant with type");
                                                    variants.push(format!(
                                                        "        {}({})",
                                                        variant_name, ty
                                                    ));
                                                }
                                                Err(e) => {
                                                    warn!(enum_name = %name, variant_name = %variant_orig_name, error = %e, "Error converting variant type");
                                                    // Propagate error immediately
                                                    return Err(e.wrap_err(format!("Failed to convert variant '{}' in enum '{}'", variant_orig_name, name)));
                                                }
                                            }
                                        }
                                        syn::Fields::Unit => {
                                            let variant_name = to_kebab_case(&variant_orig_name);
                                            debug!(original_name = %variant_orig_name, kebab_name = %variant_name, "Found unit enum variant");
                                            variants.push(format!("        {}", variant_name));
                                        }
                                        _ => {
                                            warn!(enum_name = %name, variant_name = %variant_orig_name, "Skipping complex variant in used enum");
                                            skip_enum = true;
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(enum_name = %name, error = %e, "Skipping variant with invalid name in used enum");
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
                            warn!(name = %name, "Skipping used enum due to complex/invalid variants or no variants");
                        }
                    }
                    Err(e) => {
                        // Enum name validation failed, skip regardless of usage
                        warn!(error = %e, "Skipping enum with invalid name");
                        continue;
                    }
                }
            }
            _ => {} // Handle other top-level items like functions, impls, etc. if needed
        }
    }

    debug!(
        count = %type_defs.len(), file_path = %file_path.display(),
        "Collected used type definitions from this file"
    );
    Ok(type_defs) // Return the collected definitions for this file
}

// Find all relevant Rust projects
fn find_rust_projects(base_dir: &Path) -> Vec<PathBuf> {
    let mut projects = Vec::new();
    debug!(base_dir = %base_dir.display(), "Scanning for Rust projects");

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
        debug!(path = %cargo_toml.display(), "Checking path");

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
            warn!(path = %cargo_toml.display(), "No package.metadata.component metadata found");
            continue;
        };
        let Some(package) = metadata.get("package") else {
            continue;
        };
        let Some(package_str) = package.as_str() else {
            continue;
        };
        debug!(package = %package_str, "Found package.metadata.component.package");
        if package_str == "hyperware:process" {
            debug!(path = %path.display(), "Adding project");
            projects.push(path.to_path_buf());
        }
    }

    debug!(count = %projects.len(), "Found relevant Rust projects");
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
                        let param_name = check_and_strip_leading_underscore(param_orig_name);
                        let param_name = to_kebab_case(&param_name);

                        // Rust type to WIT type
                        match rust_type_to_wit(&pat_type.ty, used_types) {
                            Ok(param_type) => {
                                // Add field directly to the struct
                                struct_fields
                                    .push(format!("        {}: {}", param_name, param_type));
                            }
                            Err(e) => {
                                warn!(param_name = %param_name, error = %e, "Error converting parameter type");
                                return Err(e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Skipping parameter with invalid name");
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
                warn!(struct_name = %signature_struct_name, error = %e, "Error converting return type");
                return Err(e);
            }
        },
        syn::ReturnType::Default => {
            // This corresponds to -> () or no return type
            // Use tuple<> for functions returning nothing (Rust unit type)
            struct_fields.push("        returning: tuple<>".to_string());
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
    debug!(project_path = %project_path.display(), "Processing project");

    // Find lib.rs for this project
    let lib_rs = project_path.join("src").join("lib.rs");

    if !lib_rs.exists() {
        warn!(project_path = %project_path.display(), "No lib.rs found for project");
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
                    debug!(wit_world = %world_name, "Extracted wit_world");
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
                        warn!(interface_name = %name, error = %e, "Interface name validation failed");
                        continue; // Skip this impl block if validation fails
                    }

                    // Remove State suffix if present
                    let base_name = remove_state_suffix(name);

                    // Convert to kebab-case for file name and interface name
                    kebab_interface_name = Some(to_kebab_case(&base_name));

                    debug!(interface_name = ?interface_name, base_name = %base_name, kebab_name = ?kebab_interface_name, "Interface details");

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
    let mut used_types = HashSet::new(); // This will be populated by signatures AND nested types

    // Analyze the functions within the identified impl block (if found)
    if let Some(ref impl_item) = &impl_item_with_hyperprocess {
        if let Some(ref _kebab_name) = &kebab_interface_name {
            // Ensure kebab_name is available but acknowledge unused in this block
            for item in &impl_item.items {
                let ImplItem::Fn(method) = item else {
                    continue;
                };
                let method_name = method.sig.ident.to_string();
                debug!(method_name = %method_name, "Examining method");

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
                let has_init = method.attrs.iter().any(|attr| attr.path().is_ident("init"));

                if has_remote || has_local || has_http || has_init {
                    debug!(remote = %has_remote, local = %has_local, http = %has_http, init = %has_init, "Method attributes");

                    // Validate function name
                    match validate_name(&method_name, "Function") {
                        Ok(_) => {
                            // Convert function name to kebab-case
                            let func_kebab_name = to_kebab_case(&method_name); // Use different var name

                            debug!(original_name = %method_name, kebab_name = %func_kebab_name, "Processing method");

                            if has_init {
                                debug!(method_name = %method_name, "Found initialization function");
                                continue;
                            }
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
                                        warn!(method_name = %method_name, error = %e, "Error generating remote signature struct");
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
                                        warn!(method_name = %method_name, error = %e, "Error generating local signature struct");
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
                                        warn!(method_name = %method_name, error = %e, "Error generating HTTP signature struct");
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("    Skipping method with invalid name: {}", e);
                            warn!(method_name = %method_name, error = %e, "Skipping method with invalid name");
                        }
                    }
                } else {
                    warn!("   Method {} does not have the [remote], [local], [http] or [init] attribute, it should not be in the Impl block", method_name);
                    warn!(method_name = %method_name, "Method missing required attribute ([remote], [local], [http], or [init])");
                }
            }
        }
    }

    // Collect **only used** type definitions from all Rust files
    let mut all_type_defs = HashMap::new();
    for file_path in &rust_files {
        // Pass the populated used_types set to the collector, making it mutable
        // It will ADD nested types to `used_types` if they are custom.
        match collect_type_definitions_from_file(file_path, &mut used_types) {
            // Pass as mutable
            Ok(file_type_defs) => {
                for (name, def) in file_type_defs {
                    // Insert definition if successfully generated. We might insert the same
                    // definition multiple times if defined in multiple files; HashMap handles this.
                    // Crucially, we only insert if the struct/enum *itself* was initially in used_types.
                    all_type_defs.insert(name, def);
                }
            }
            Err(e) => {
                // Decide how to handle errors from collection: warn and continue, or bail?
                // Bailing might be safer to ensure correctness.
                warn!(file_path = %file_path.display(), error = %e, "Error collecting type definitions from file, WIT may be incomplete.");
                // For stricter checking, you might uncomment the next line:
                return Err(e.wrap_err(format!(
                    "Failed to collect type definitions from {}",
                    file_path.display()
                )));
            }
        }
    }

    // The verification logic added previously now works correctly because
    // `used_types` contains ALL custom type names encountered, both top-level and nested.
    debug!(used_type_count = %used_types.len(), used_types = ?used_types, "Final set of used types for verification"); // Added debug

    // Verify that all used types are either primitive/builtin or defined locally
    let mut undefined_types = Vec::new();
    for used_type_name in &used_types {
        // Check if the used type is a primitive/builtin OR if we found its definition locally
        if !is_wit_primitive_or_builtin(used_type_name)
            && !all_type_defs.contains_key(used_type_name)
        {
            undefined_types.push(used_type_name.clone());
        }
    }

    // If there are undefined types, raise an error
    if !undefined_types.is_empty() {
        undefined_types.sort();
        bail!(
            "WIT Generation Error in project '{}': Found types used (directly or indirectly) in function signatures \
             that are neither WIT built-ins nor defined locally within the scanned project files: {:?}. \
             Ensure definitions for these types (structs/enums) are present in the project's source code, \
             or adjust the function/type definitions to use only WIT-compatible types.",
             project_path.display(),
             undefined_types
        );
    }
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

        // Generate the final WIT content
        if signature_structs.is_empty() && type_defs.is_empty() {
            // Check both sigs and types
            warn!(interface_name = %iface_name, "No functions or used types found for interface");
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
            debug!(interface_name = %iface_name, signature_count = %signature_structs.len(), type_def_count = %type_defs.len(), "Generated interface content");

            // Write the interface file with kebab-case name
            let interface_file = api_dir.join(format!("{}.wit", kebab_name));
            debug!(path = %interface_file.display(), "Writing WIT file");

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
        debug!(interface = %kebab_iface, "Returning import statement for interface");
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
    debug!(api_dir = %api_dir.display(), "Rewriting WIT world files");
    // handle existing api files
    for entry in WalkDir::new(api_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "wit") {
            debug!(path = %path.display(), "Checking WIT file");

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

            debug!(world_name = %world_name, "Extracted world name");

            // Check if this world name matches the one we're looking for
            if wit_worlds.remove(&world_name) || wit_worlds.contains(&world_name[6..]) {
                let world_content = generate_wit_file(
                    &world_name,
                    new_imports,
                    &existing_imports,
                    &mut include_lines,
                )?;

                debug!(path = %path.display(), "Writing updated world definition");
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
            debug!(path = %path.display(), wit_world = %wit_world, "Writing new world definition");
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
            Ok(None) => {
                bail!("No import statement generated for project {}", project_path.display());
            }
            Err(e) => {
                bail!(
                    "Error processing project {}: {}",
                    project_path.display(),
                    e
                );
            }
        }
    }

    debug!(count = %new_imports.len(), "Collected number of new imports");

    // Check for existing world definition files and update them
    debug!("Looking for existing world definition files");
    let mut updated_world = false;

    rewrite_wit(api_dir, &new_imports, &mut wit_worlds, &mut updated_world)?;

    // If no world definitions were found, create a default one
    if !updated_world && !new_imports.is_empty() {
        // Define default world name
        let default_world = "async-app-template-dot-os-v0";
        warn!(default_world = %default_world, "No existing world definitions found, creating default");

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
        debug!(path = %world_file.display(), "Writing default world definition");

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
