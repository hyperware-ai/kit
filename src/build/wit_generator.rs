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
         "      Warning: Field name starts with an underscore ('_'), which is invalid in WIT. Stripping the underscore from WIT definition. Function signatures should only include parameters that are actually used."
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
                        // Strictly enforce exactly two arguments for Result<T, E>
                        if args.args.len() == 2 {
                            if let (
                                Some(syn::GenericArgument::Type(ok_ty)),
                                Some(syn::GenericArgument::Type(err_ty)),
                            ) = (args.args.first(), args.args.get(1))
                            {
                                let ok_type_str = rust_type_to_wit(ok_ty, used_types)?;
                                let err_type_str = rust_type_to_wit(err_ty, used_types)?;

                                // Map Rust's () (represented as "_") to WIT's _ in result<...>
                                let final_ok = if ok_type_str == "_" {
                                    // Check for "_"
                                    "_"
                                } else {
                                    &ok_type_str
                                };
                                let final_err = if err_type_str == "_" {
                                    // Check for "_"
                                    "_"
                                } else {
                                    &err_type_str
                                };

                                // Format the WIT result string according to WIT conventions
                                let result_string = match (final_ok, final_err) {
                                    ("_", "_") => "result".to_string(),          // Shorthand: result
                                    (ok, "_") => format!("result<{}>", ok), // Shorthand: result<T>
                                    ("_", err) => format!("result<_, {}>", err), // Explicit: result<_, E>
                                    (ok, err) => format!("result<{}, {}>", ok, err), // Explicit: result<T, E>
                                };
                                Ok(result_string)
                            } else {
                                // This case should be unlikely if len == 2, but handle defensively
                                Err(eyre!("Failed to parse Result generic arguments"))
                            }
                        } else {
                            Err(eyre!(
                                "Result requires exactly two type arguments (e.g., Result<T, E>), found {}",
                                args.args.len()
                            ))
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
        // fn () -> Result<(), Error>
        // tuple<>
        Type::Tuple(type_tuple) => {
            if type_tuple.elems.is_empty() {
                // Represent () as "_" for the caller to interpret based on context.
                // It's valid within Result<_, E>, but invalid as a direct return type.
                Ok("_".to_string())
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

// Searches a single file for a specific type definition (struct or enum) by its kebab-case name.
// If found, generates its WIT definition string and returns it along with any new custom type
// dependencies discovered within its fields/variants.
#[instrument(level = "trace", skip_all)]
fn find_and_make_wit_type_def(
    file_path: &Path,
    target_kebab_type_name: &str,
    global_used_types: &mut HashSet<String>, // Track all used types globally
) -> Result<Option<(String, HashSet<String>)>> {
    // Return: Ok(Some((wit_def, new_local_deps))), Ok(None), or Err
    debug!(
        file_path = %file_path.display(),
        target_type = %target_kebab_type_name,
        "Searching for type definition"
    );

    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

    let ast = syn::parse_file(&content)
        .with_context(|| format!("Failed to parse file: {}", file_path.display()))?;

    for item in &ast.items {
        // Determine if the current item matches the target type name
        let (is_target, item_kind, orig_name) = match item {
            Item::Struct(s) => {
                let name = s.ident.to_string();
                (
                    to_kebab_case(&name) == target_kebab_type_name,
                    "Struct",
                    name,
                )
            }
            Item::Enum(e) => {
                let name = e.ident.to_string();
                (to_kebab_case(&name) == target_kebab_type_name, "Enum", name)
            }
            _ => (false, "", String::new()),
        };

        if is_target {
            // Skip internal-looking types (can be adjusted)
            if orig_name.contains("__") {
                warn!(name = %orig_name, "Skipping definition search for likely internal type");
                return Ok(None); // Treat as not found for WIT purposes
            }
            // Validate the original Rust name
            validate_name(&orig_name, item_kind)?;

            let kebab_name = target_kebab_type_name; // We know this matches
            let mut local_dependencies = HashSet::new(); // Track deps discovered *by this type*

            // --- Generate Struct Definition ---
            if let Item::Struct(item_struct) = item {
                let fields_result: Result<Vec<String>> = match &item_struct.fields {
                    syn::Fields::Named(fields) => {
                        let mut field_strings = Vec::new();
                        for f in &fields.named {
                            if let Some(field_ident) = &f.ident {
                                let field_orig_name = field_ident.to_string();
                                // Validate field name (allow underscore stripping)
                                let stripped_field_orig_name =
                                    check_and_strip_leading_underscore(field_orig_name.clone());
                                // Validate the potentially stripped name, adding context about the rules
                                validate_name(&stripped_field_orig_name, "Field")?;

                                let field_kebab_name = to_kebab_case(&stripped_field_orig_name);
                                if field_kebab_name.is_empty() {
                                     warn!(struct_name=%kebab_name, field_original_name=%field_orig_name, "Skipping field with empty kebab-case name");
                                    continue;
                                }

                                // Convert field type. `rust_type_to_wit` adds any new custom types
                                // found within the field type (e.g., in list<T>) to `global_used_types`.
                                let field_wit_type = rust_type_to_wit(&f.ty, global_used_types)
                                    .wrap_err_with(|| format!("Failed to convert field '{}':'{:?}' in struct '{}'", field_orig_name, f.ty, orig_name))?;

                                // If the resulting WIT type itself is custom, add it to *local* dependencies
                                // so the caller knows this struct definition depends on it.
                                if !is_wit_primitive_or_builtin(&field_wit_type) {
                                    local_dependencies.insert(field_wit_type.clone());
                                }

                                field_strings.push(format!("        {}: {}", field_kebab_name, field_wit_type));
                            }
                        }
                        Ok(field_strings)
                    }
                    // Handle Unit Structs as empty records
                    syn::Fields::Unit => Ok(Vec::new()),
                    // Decide how to handle Tuple Structs (e.g., error, skip, specific WIT representation?)
                    syn::Fields::Unnamed(_) => bail!("Tuple structs ('struct {} (...)') are not currently supported for WIT generation.", orig_name),
                };

                match fields_result {
                    Ok(fields_vec) => {
                        // Generate record definition (use {} for empty records)
                        let definition = if fields_vec.is_empty() {
                            format!("    record {} {{}}", kebab_name)
                        } else {
                            format!(
                                "    record {} {{\n{}\n    }}",
                                kebab_name,
                                fields_vec.join(",\n")
                            )
                        };
                        debug!(type_name = %kebab_name, "Generated record definition");
                        return Ok(Some((definition, local_dependencies)));
                    }
                    Err(e) => return Err(e), // Propagate field processing error
                }
            }

            // --- Generate Enum Definition ---
            if let Item::Enum(item_enum) = item {
                let mut variants_wit = Vec::new();
                let mut skip_enum = false;

                for v in &item_enum.variants {
                    let variant_orig_name = v.ident.to_string();
                    // Validate variant name before proceeding
                    validate_name(&variant_orig_name, "Enum variant")?;
                    let variant_kebab_name = to_kebab_case(&variant_orig_name);

                    match &v.fields {
                        // Variant with one unnamed field: T -> case(T)
                        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                            // `rust_type_to_wit` adds new custom types to `global_used_types`
                            let type_result = rust_type_to_wit(
                                &fields.unnamed.first().unwrap().ty,
                                global_used_types,
                            )
                            .wrap_err_with(|| {
                                format!(
                                    "Failed to convert variant '{}' type in enum '{}'",
                                    variant_orig_name, orig_name
                                )
                            })?;

                            // Check if the variant's type is custom and add to local deps
                            if !is_wit_primitive_or_builtin(&type_result) {
                                local_dependencies.insert(type_result.clone());
                            }
                            variants_wit
                                .push(format!("        {}({})", variant_kebab_name, type_result));
                        }
                        // Unit variant: -> case
                        syn::Fields::Unit => {
                            variants_wit.push(format!("        {}", variant_kebab_name));
                        }
                        // Variants with named fields or multiple unnamed fields are not directly supported by WIT variants
                        _ => {
                            warn!(enum_name = %kebab_name, variant_name = %variant_orig_name, "Skipping complex enum variant (only unit variants or single-type variants like 'MyVariant(MyType)' are supported)");
                            skip_enum = true;
                            break; // Skip the whole enum if one variant is complex
                        }
                    }
                }

                // Only generate if not skipped and has convertible variants
                if !skip_enum && !variants_wit.is_empty() {
                    let definition = format!(
                        "    variant {} {{\n{}\n    }}",
                        kebab_name,
                        variants_wit.join(",\n")
                    );
                    debug!(type_name = %kebab_name, "Generated variant definition");
                    return Ok(Some((definition, local_dependencies)));
                } else {
                    // Treat as not found for WIT generation if skipped or empty
                    warn!(name = %kebab_name, "Skipping enum definition due to complex/invalid variants or no convertible variants");
                    return Ok(None);
                }
            }
            // Should not be reached if item is Struct or Enum and is_target is true
            unreachable!("Target type matched but was neither struct nor enum?");
        }
    }

    // Target type definition was not found in this specific file
    Ok(None)
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

                // Get original param name
                let param_orig_name = pat_ident.ident.to_string();
                let method_name_for_error = method.sig.ident.to_string(); // Get method name for error messages

                // Validate parameter name
                match validate_name(&param_orig_name, "Parameter") {
                    Ok(_) => {
                        let stripped_param_name =
                            check_and_strip_leading_underscore(param_orig_name.clone()); // Clone needed
                        let param_name = to_kebab_case(&stripped_param_name);

                        // Rust type to WIT type
                        match rust_type_to_wit(&pat_type.ty, used_types) {
                            Ok(param_type) => {
                                // Add field directly to the struct
                                struct_fields
                                    .push(format!("        {}: {}", param_name, param_type));
                            }
                            Err(e) => {
                                // Wrap parameter type conversion error with context
                                return Err(e.wrap_err(format!(
                                    "Failed to convert type for parameter '{}' in function '{}'",
                                    param_orig_name, method_name_for_error
                                )));
                            }
                        }
                    }
                    Err(e) => {
                        // Return the error directly
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
                // Check if the return type is "_", which signifies a standalone () return type.
                if return_type == "_" {
                    let method_name = method.sig.ident.to_string();
                    bail!(
                        "Function '{}' returns '()', which is not directly supported in WIT signatures. \
                         Consider returning a Result<(), YourErrorType> or another meaningful type.",
                        method_name
                    );
                }
                // Add the valid return type field
                struct_fields.push(format!("        returning: {}", return_type));
            }
            Err(e) => {
                // Propagate *other* errors from return type conversion, wrapping them.
                let method_name = method.sig.ident.to_string();
                return Err(e.wrap_err(format!(
                    "Failed to convert return type for function '{}'",
                    method_name
                )));
            }
        },
        syn::ReturnType::Default => {
            // Functions exposed via WIT must have an explicit return type.
            let method_name = method.sig.ident.to_string();
            bail!(
                "Function '{}' must have an explicit return type (e.g., '-> MyType' or '-> Result<(), YourErrorType>') to be exposed via WIT. Implicit return types are not allowed.",
                method_name
            );
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

    // --- 0. Setup & Find Project Files ---
    let lib_rs = project_path.join("src").join("lib.rs");
    if !lib_rs.exists() {
        warn!(project_path = %project_path.display(), "No lib.rs found, skipping project");
        return Ok(None);
    }
    let rust_files = find_rust_files(project_path);
    if rust_files.is_empty() {
        warn!(project_path=%project_path.display(), "No Rust files found in src/, skipping project");
        return Ok(None);
    }
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

    // --- 1. Find Hyperprocess Impl Block & Extract Metadata ---
    let mut wit_world = None;
    let mut interface_name = None; // Original Rust name (e.g., MyProcessState)
    let mut kebab_interface_name = None; // Kebab-case name (e.g., my-process)
    let mut impl_item_with_hyperprocess = None;

    debug!("Scanning lib.rs for impl block with #[hyperprocess] attribute");
    for item in &ast.items {
        if let Item::Impl(impl_item) = item {
            if let Some(attr) = impl_item
                .attrs
                .iter()
                .find(|a| a.path().is_ident("hyperprocess"))
            {
                debug!("Found #[hyperprocess] attribute");
                // Attempt to extract wit_world. Propagate error if extraction fails.
                let world_name = extract_wit_world(&[attr.clone()])
                    .wrap_err("Failed to extract wit_world from #[hyperprocess] attribute")?;
                debug!(wit_world = %world_name, "Extracted wit_world");
                wit_world = Some(world_name);


                // Get the struct name from the 'impl MyStruct for ...' part
                interface_name = impl_item.self_ty.as_ref().as_type_path().and_then(|tp| {
                    tp.path.segments.last().map(|seg| seg.ident.to_string())
                });

                if let Some(ref name) = interface_name {
                    // Validate original name first
                     match validate_name(name, "Interface") {
                        Ok(_) => {
                            let base_name = remove_state_suffix(name);
                            kebab_interface_name = Some(to_kebab_case(&base_name));
                            debug!(interface_name = %name, base_name = %base_name, kebab_name = ?kebab_interface_name, "Interface details");
                            impl_item_with_hyperprocess = Some(impl_item.clone());
                            break; // Found the target impl block
                        }
                        Err(e) => {
                            // Escalate errors for invalid interface names instead of just warning
                            return Err(e.wrap_err(format!("Invalid interface name '{}' in hyperprocess impl block", name)));
                        }
                    }
                } else {
                    // If interface name couldn't be extracted, it's an error for this project.
                    bail!("Could not extract interface name from #[hyperprocess] impl block type: {:?}", impl_item.self_ty);

                }


            }
        }
    }

    // Exit early if no valid hyperprocess impl block was identified
    let Some(ref impl_item) = impl_item_with_hyperprocess else {
        // If we looped through everything and didn't find a block (and didn't error above),
        // it means no #[hyperprocess] attribute was found at all. This is okay, just skip.
        warn!(project_path=%project_path.display(), "No #[hyperprocess] impl block found in lib.rs, skipping project");
        return Ok(None);
    };
    // These unwraps are safe due to the checks above ensuring we error or break successfully
    let kebab_name = kebab_interface_name.as_ref().unwrap();
    let current_wit_world = wit_world.as_ref().unwrap();

    // --- 2. Collect Signatures & Initial Types ---
    let mut signature_structs = Vec::new(); // Stores WIT string for each signature record
    let mut global_used_types = HashSet::new(); // All custom WIT types encountered (kebab-case)

    debug!("Analyzing functions in hyperprocess impl block");
    for item in &impl_item.items {
        if let ImplItem::Fn(method) = item {
            let method_name = method.sig.ident.to_string();
            debug!(method_name = %method_name, "Examining method");

            let has_remote = method.attrs.iter().any(|a| a.path().is_ident("remote"));
            let has_local = method.attrs.iter().any(|a| a.path().is_ident("local"));
            let has_http = method.attrs.iter().any(|a| a.path().is_ident("http"));
            let has_init = method.attrs.iter().any(|a| a.path().is_ident("init"));

            if has_remote || has_local || has_http || has_init {
                debug!(remote=%has_remote, local=%has_local, http=%has_http, init=%has_init, "Method attributes found");
                // Validate original Rust function name
                validate_name(&method_name, "Function")?; // Error early if name invalid
                let func_kebab_name = to_kebab_case(&method_name);

                if has_init {
                    debug!(method_name = %method_name, "Found [init] function, skipping signature generation");
                    continue;
                }

                // Generate signature structs. `generate_signature_struct` calls `rust_type_to_wit`,
                // which populates `global_used_types` with all custom types found in parameters/return types.
                if has_remote {
                    let sig_struct = generate_signature_struct(
                        &func_kebab_name,
                        "remote",
                        method,
                        &mut global_used_types,
                    )?;
                    signature_structs.push(sig_struct);
                }
                if has_local {
                    let sig_struct = generate_signature_struct(
                        &func_kebab_name,
                        "local",
                        method,
                        &mut global_used_types,
                    )?;
                    signature_structs.push(sig_struct);
                }
                if has_http {
                    let sig_struct = generate_signature_struct(
                        &func_kebab_name,
                        "http",
                        method,
                        &mut global_used_types,
                    )?;
                    signature_structs.push(sig_struct);
                }
            } else {
                // Method in hyperprocess impl lacks required attribute - Error
                return Err(eyre!(
                         "Method '{}' in the #[hyperprocess] impl block is missing a required attribute ([remote], [local], [http], or [init]). Only methods with these attributes should be included.",
                         method_name
                     ));
            }
        }
    }
    debug!(signature_count = %signature_structs.len(), initial_used_types = ?global_used_types, "Completed signature analysis");

    // --- 3. Resolve & Generate Type Definitions Iteratively ---
    debug!("Starting iterative type definition resolution");
    let mut generated_type_defs = HashMap::new(); // Kebab-case name -> WIT definition string
    let mut types_to_find_queue: Vec<String> = global_used_types // Initialize queue
        .iter()
        .filter(|ty| !is_wit_primitive_or_builtin(ty)) // Only custom types
        .cloned()
        .collect();
    let mut processed_types = HashSet::new(); // Track types processed to avoid cycles/redundancy

    // Add primitives/builtins to processed_types initially
    for ty in &global_used_types {
        if is_wit_primitive_or_builtin(ty) {
            processed_types.insert(ty.clone());
        }
    }

    while let Some(type_name_to_find) = types_to_find_queue.pop() {
        if processed_types.contains(&type_name_to_find) {
            continue; // Already processed or known primitive/builtin
        }

        debug!(type_name = %type_name_to_find, "Attempting to find definition");
        let mut definition_found_in_project = false;

        // Search across all project files for the definition
        for file_path in &rust_files {
            // Directly propagate errors from find_and_make_wit_type_def
            match find_and_make_wit_type_def(file_path, &type_name_to_find, &mut global_used_types)? {
                Some((wit_definition, new_local_deps)) => {


                    debug!(type_name=%type_name_to_find, file_path=%file_path.display(), "Found definition");

                    // Store the definition. Check for duplicates across files.
                    if let Some(existing_def) = generated_type_defs.insert(type_name_to_find.clone(), wit_definition.clone()) { // Clone wit_definition here
                         // Simple string comparison might be too strict if formatting differs slightly.
                         // But good enough for a warning.
                         if existing_def != wit_definition { // Compare with the cloned value
                            warn!(type_name = %type_name_to_find, "Type definition found in multiple files with different generated content. Using the one from: {}", file_path.display());
                        }
                    }
                    processed_types.insert(type_name_to_find.clone()); // Mark as processed
                    definition_found_in_project = true;

                    // Add newly discovered dependencies from this type's definition to the queue
                    for dep in new_local_deps {
                        if !processed_types.contains(&dep) && !types_to_find_queue.contains(&dep) {
                            debug!(dependency = %dep, discovered_by = %type_name_to_find, "Adding new dependency to find queue");
                            types_to_find_queue.push(dep);
                        }
                    }
                    // Found the definition for this type, stop searching files for it
                    break;
                }
                None => continue, // Not in this file, check next file


            }
        }
        // If after checking all files, the definition wasn't found
        if !definition_found_in_project {
            debug!(type_name=%type_name_to_find, "Definition not found in any scanned file.");
            // Mark as processed to avoid infinite loop. Verification step will catch this.
            processed_types.insert(type_name_to_find.clone());
        }
    }
    debug!("Finished iterative type definition resolution");

    // --- 4. Verify All Used Types Have Definitions ---
    debug!(final_used_types = ?global_used_types, found_definitions = ?generated_type_defs.keys(), "Starting final verification");
    let mut undefined_types = Vec::new();
    for used_type_name in &global_used_types {
        if !is_wit_primitive_or_builtin(used_type_name)
            && !generated_type_defs.contains_key(used_type_name)
        {
            warn!(type_name=%used_type_name, "Verification failed: Used type has no generated definition.");
            undefined_types.push(used_type_name.clone());
        }
    }

    if !undefined_types.is_empty() {
        undefined_types.sort();
        // Use the original project path display for user-friendliness
        let project_display = project_path.display();
        bail!(
            "WIT Generation Error in project '{}': Found types used (directly or indirectly) in function signatures \
             that are neither WIT built-ins nor defined locally within the scanned project files: {:?}. \
             Ensure definitions for these types (structs/enums) are present in the project's source code \
             (and not skipped due to errors/complexity), or adjust the function/type definitions.",
             project_display,
             undefined_types
        );
    }
    debug!("Verification successful: All used types have definitions or are built-in.");

    // --- 5. Generate Final WIT Interface File ---
    let mut all_generated_defs: Vec<String> = generated_type_defs.into_values().collect();
    all_generated_defs.sort(); // Sort type definitions for consistent output
    signature_structs.sort(); // Sort signature records as well

    if signature_structs.is_empty() && all_generated_defs.is_empty() {
        // Use the original interface name if available, otherwise fallback
        let name_for_warning = interface_name.as_deref().unwrap_or("<unknown>");
        warn!(interface_name = %name_for_warning, "No attributed functions or used types requiring definitions found. No WIT interface file generated for this project.");

         // Return the world name even if no interface content is generated,
         // so the world file can still be updated/created if necessary.
         // But signal that no *interface* was generated by returning None for the interface name part.
        return Ok(Some((String::new(), current_wit_world.to_string()))); // Return empty string for interface name
    } else {
        debug!(kebab_name=%kebab_name, "Generating final WIT content");
        let mut content = String::new();

        // Add standard imports (can be refined based on actual needs)
        content.push_str("    use standard.{address};\n"); // Assuming world includes 'standard'

        // Add type definitions
        if !all_generated_defs.is_empty() {
            content.push('\n'); // Separator
            debug!(count=%all_generated_defs.len(), "Adding type definitions to interface");
            content.push_str(&all_generated_defs.join("\n\n"));
            content.push('\n');
        }

        // Add signature structs
        if !signature_structs.is_empty() {
            content.push('\n'); // Separator
            debug!(count=%signature_structs.len(), "Adding signature structs to interface");
            content.push_str(&signature_structs.join("\n\n"));
        }

        // Wrap in interface block
        let final_content = format!("interface {} {{\n{}\n}}\n", kebab_name, content.trim()); // Trim any trailing whitespace
        debug!(interface_name = %interface_name.as_ref().unwrap(), signature_count = %signature_structs.len(), type_def_count = %all_generated_defs.len(), "Generated interface content");

        // Write the interface file
        let interface_file = api_dir.join(format!("{}.wit", kebab_name));
        debug!(path = %interface_file.display(), "Writing WIT file");
        fs::write(&interface_file, &final_content).with_context(|| {
            format!(
                "Failed to write WIT interface file: {}",
                interface_file.display()
            )
        })?;
        debug!("Successfully wrote WIT file");

        // If content was generated, return the kebab name for the import statement
        debug!(interface = %kebab_name, wit_world=%current_wit_world, "Returning import statement info");
        Ok(Some((
            kebab_name.to_string(),
            current_wit_world.to_string(),
        )))
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
    let mut interfaces = Vec::new(); // Kebab-case interface names

    let mut wit_worlds = HashSet::new(); // Collect all unique world names encountered
    for project_path in &projects {
        match process_rust_project(project_path, api_dir) {
            // Project processed successfully, yielding an interface name and world name
            Ok(Some((interface, wit_world))) => {
                // Only add import if an interface name was actually generated
                if !interface.is_empty() {
                    new_imports.push(format!("    import {interface};"));
                    interfaces.push(interface); // Add to list of generated interfaces
                } else {
                     // Log if processing succeeded but generated no interface content
                     debug!(project = %project_path.display(), world = %wit_world, "Project processed but generated no interface content (only types/no functions?)");
                }
                // Always record the project path and the target world
                processed_projects.push(project_path.clone());
                wit_worlds.insert(wit_world);
            }
            // Project was skipped intentionally (e.g., no lib.rs, no #[hyperprocess])
            Ok(None) => {
                debug!(project = %project_path.display(), "Project skipped during processing (e.g., no lib.rs or #[hyperprocess] found)");
                // Continue to the next project
                continue;
            }
            // An error occurred during processing
            Err(e) => {
                // Propagate the error, stopping the entire generation process
                bail!("Error processing project {}: {}", project_path.display(), e);
            }
        }
    }


    debug!(count = %new_imports.len(), "Collected number of new imports");
    if new_imports.is_empty() && wit_worlds.is_empty() {
        info!("No WIT interfaces generated and no target WIT worlds identified across all projects.");
        return Ok((processed_projects, interfaces)); // Return empty interfaces list
    } else if new_imports.is_empty() {
        info!("No new WIT interfaces generated, but target WIT world(s) identified: {:?}", wit_worlds);
        // Proceed to rewrite world files even without new imports, as existing ones might need updates/creation.
    }


    // Update or create WIT world files
    debug!("Processing WIT world files for: {:?}", wit_worlds);
    let mut updated_world = false; // Track if any world file was written/updated


    rewrite_wit(api_dir, &new_imports, &mut wit_worlds.clone(), &mut updated_world)?; // Pass a clone as rewrite_wit might modify it


    // If no existing world matched and no new world files were created by rewrite_wit,
    // AND we actually had imports to add, create a default world file.
    if !updated_world && !new_imports.is_empty() {

        // Define default world name
        let default_world = "async-app-template-dot-os-v0";
        warn!(default_world = %default_world, "No existing world definitions found or created for collected imports, creating default world file");

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

        let mut includes = HashSet::new();
        includes.insert(include_line.to_string());

        // Generate content using the helper function
        let world_content = generate_wit_file(default_world, &new_imports, &Vec::new(), &mut includes)?;


        let world_file = api_dir.join(format!("{}.wit", default_world));
        debug!(path = %world_file.display(), "Writing default world definition");

        fs::write(&world_file, world_content).with_context(|| {
            format!(
                "Failed to write default world file: {}",
                world_file.display()
            )
        })?;

        debug!("Successfully created default world definition");
        updated_world = true; // Mark that a world file was indeed created
    } else if !updated_world {
        info!("No world files were updated or created (either no imports needed adding, or target worlds already existed and were up-to-date).");
    }


    info!("WIT file generation process completed.");
    Ok((processed_projects, interfaces)) // Return list of successfully processed projects and generated interfaces
}
