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

// List of WIT keywords that need to be prefixed with %
fn is_wit_keyword(s: &str) -> bool {
    matches!(
        s,
        "use"
            | "type"
            | "resource"
            | "func"
            | "record"
            | "enum"
            | "flags"
            | "variant"
            | "static"
            | "interface"
            | "world"
            | "import"
            | "export"
            | "package"
            | "constructor"
            | "include"
            | "with"
            | "as"
            | "from"
            | "list"
            | "option"
            | "result"
            | "tuple"
            | "future"
            | "stream"
            | "own"
            | "borrow"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "s8"
            | "s16"
            | "s32"
            | "s64"
            | "f32"
            | "f64"
            | "char"
            | "bool"
            | "string"
    )
}

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

// Convert a name to valid WIT identifier, prefixing with % if it's a keyword
fn to_wit_ident(kebab_name: &str) -> String {
    if is_wit_keyword(kebab_name) {
        format!("%{}", kebab_name)
    } else {
        kebab_name.to_string()
    }
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
    let mut comment = format!(
        "    // Function signature for: {} ({})",
        kebab_name, attr_type
    );

    // For HTTP endpoints, try to extract method and path from attribute
    if attr_type == "http" {
        if let Some((http_method, http_path)) = extract_http_info(&method.attrs)? {
            comment.push_str(&format!("\n    // HTTP: {} {}", http_method, http_path));
        } else {
            // Default path if not specified
            comment.push_str(&format!("\n    // HTTP: POST /api/{}", kebab_name));
        }
    }

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
                        let param_wit_ident = to_wit_ident(&param_name);

                        // Rust type to WIT type
                        match rust_type_to_wit(&pat_type.ty, used_types) {
                            Ok(param_type) => {
                                // Add field directly to the struct
                                struct_fields
                                    .push(format!("        {}: {}", param_wit_ident, param_type));
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

    // HTTP handlers no longer require parameters - they can have zero parameters

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

// Helper function to extract HTTP method and path from [http] attribute
#[instrument(level = "trace", skip_all)]
fn extract_http_info(attrs: &[Attribute]) -> Result<Option<(String, String)>> {
    for attr in attrs {
        if attr.path().is_ident("http") {
            // Convert attribute to string representation for parsing
            let attr_str = format!("{:?}", attr);
            debug!(attr_str = %attr_str, "HTTP attribute string");

            let mut method = None;
            let mut path = None;

            // Look for method parameter
            if let Some(method_pos) = attr_str.find("method") {
                if let Some(eq_pos) = attr_str[method_pos..].find('=') {
                    let start_pos = method_pos + eq_pos + 1;
                    // Find the quoted value
                    if let Some(quote_start) = attr_str[start_pos..].find('"') {
                        let value_start = start_pos + quote_start + 1;
                        if let Some(quote_end) = attr_str[value_start..].find('"') {
                            method =
                                Some(attr_str[value_start..value_start + quote_end].to_string());
                        }
                    }
                }
            }

            // Look for path parameter
            if let Some(path_pos) = attr_str.find("path") {
                if let Some(eq_pos) = attr_str[path_pos..].find('=') {
                    let start_pos = path_pos + eq_pos + 1;
                    // Find the quoted value
                    if let Some(quote_start) = attr_str[start_pos..].find('"') {
                        let value_start = start_pos + quote_start + 1;
                        if let Some(quote_end) = attr_str[value_start..].find('"') {
                            path = Some(attr_str[value_start..value_start + quote_end].to_string());
                        }
                    }
                }
            }

            // If we found at least one parameter, return the info
            if method.is_some() || path.is_some() {
                let final_method = method.unwrap_or_else(|| "POST".to_string());
                let final_path = path.unwrap_or_else(|| "/api".to_string());
                return Ok(Some((final_method, final_path)));
            }
        }
    }
    Ok(None)
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

// Helper function to collect all type definitions from a file
#[instrument(level = "trace", skip_all)]
fn collect_type_definitions_from_file(
    file_path: &Path,
    type_definitions: &mut HashMap<String, String>, // kebab-name -> WIT definition
) -> Result<()> {
    debug!(file_path = %file_path.display(), "Collecting type definitions from file");

    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

    let ast = syn::parse_file(&content)
        .with_context(|| format!("Failed to parse file: {}", file_path.display()))?;

    // Temporary HashSet for tracking dependencies during collection
    let mut temp_used_types = HashSet::new();

    for item in &ast.items {
        match item {
            Item::Struct(s) => {
                let name = s.ident.to_string();
                // Skip internal types
                if name.contains("__") {
                    continue;
                }

                // Validate name
                if let Err(e) = validate_name(&name, "Struct") {
                    warn!(name = %name, error = %e, "Skipping struct with invalid name");
                    continue;
                }

                let kebab_name = to_kebab_case(&name);

                // Generate WIT definition for this struct
                let fields_result: Result<Vec<String>> = match &s.fields {
                    syn::Fields::Named(fields) => {
                        let mut field_strings = Vec::new();
                        for f in &fields.named {
                            if let Some(field_ident) = &f.ident {
                                let field_orig_name = field_ident.to_string();
                                let stripped_field_orig_name =
                                    check_and_strip_leading_underscore(field_orig_name.clone());

                                if let Err(e) = validate_name(&stripped_field_orig_name, "Field") {
                                    warn!(field_name = %field_orig_name, error = %e, "Skipping field with invalid name");
                                    continue;
                                }

                                let field_kebab_name = to_kebab_case(&stripped_field_orig_name);
                                if field_kebab_name.is_empty() {
                                    continue;
                                }

                                // Convert field type
                                match rust_type_to_wit(&f.ty, &mut temp_used_types) {
                                    Ok(field_wit_type) => {
                                        let field_wit_ident = to_wit_ident(&field_kebab_name);
                                        field_strings.push(format!(
                                            "        {}: {}",
                                            field_wit_ident, field_wit_type
                                        ));
                                    }
                                    Err(e) => {
                                        warn!(field = %field_orig_name, error = %e, "Failed to convert field type");
                                        return Err(e.wrap_err(format!(
                                            "Failed to convert field '{}' in struct '{}'",
                                            field_orig_name, name
                                        )));
                                    }
                                }
                            }
                        }
                        Ok(field_strings)
                    }
                    syn::Fields::Unit => Ok(Vec::new()),
                    syn::Fields::Unnamed(_) => {
                        warn!(struct_name = %name, "Skipping tuple struct");
                        continue;
                    }
                };

                match fields_result {
                    Ok(fields_vec) => {
                        let wit_ident = to_wit_ident(&kebab_name);
                        let definition = if fields_vec.is_empty() {
                            format!("    record {} {{}}", wit_ident)
                        } else {
                            format!(
                                "    record {} {{\n{}\n    }}",
                                wit_ident,
                                fields_vec.join(",\n")
                            )
                        };
                        type_definitions.insert(kebab_name, definition);
                    }
                    Err(e) => {
                        warn!(struct_name = %name, error = %e, "Failed to process struct");
                    }
                }
            }
            Item::Enum(e) => {
                let name = e.ident.to_string();
                // Skip internal types
                if name.contains("__") {
                    continue;
                }

                // Validate name
                if let Err(e) = validate_name(&name, "Enum") {
                    warn!(name = %name, error = %e, "Skipping enum with invalid name");
                    continue;
                }

                let kebab_name = to_kebab_case(&name);
                let mut variants_wit = Vec::new();
                let mut skip_enum = false;

                for v in &e.variants {
                    let variant_orig_name = v.ident.to_string();
                    if let Err(e) = validate_name(&variant_orig_name, "Enum variant") {
                        warn!(variant = %variant_orig_name, error = %e, "Skipping variant with invalid name");
                        skip_enum = true;
                        break;
                    }

                    let variant_kebab_name = to_kebab_case(&variant_orig_name);
                    let variant_wit_ident = to_wit_ident(&variant_kebab_name);

                    match &v.fields {
                        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                            match rust_type_to_wit(
                                &fields.unnamed.first().unwrap().ty,
                                &mut temp_used_types,
                            ) {
                                Ok(type_result) => {
                                    variants_wit.push(format!(
                                        "        {}({})",
                                        variant_wit_ident, type_result
                                    ));
                                }
                                Err(e) => {
                                    warn!(variant = %variant_orig_name, error = %e, "Failed to convert variant type");
                                    skip_enum = true;
                                    break;
                                }
                            }
                        }
                        syn::Fields::Unit => {
                            variants_wit.push(format!("        {}", variant_wit_ident));
                        }
                        _ => {
                            warn!(enum_name = %kebab_name, variant_name = %variant_orig_name, "Skipping complex enum variant");
                            skip_enum = true;
                            break;
                        }
                    }
                }

                if !skip_enum && !variants_wit.is_empty() {
                    let wit_ident = to_wit_ident(&kebab_name);
                    let definition = format!(
                        "    variant {} {{\n{}\n    }}",
                        wit_ident,
                        variants_wit.join(",\n")
                    );
                    type_definitions.insert(kebab_name, definition);
                }
            }
            _ => {}
        }
    }

    Ok(())
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

    // --- PASS 1: Collect ALL type definitions from all files first ---
    debug!("Pass 1: Collecting all type definitions from project files");
    let mut all_type_definitions = HashMap::new();

    for file_path in &rust_files {
        if let Err(e) = collect_type_definitions_from_file(file_path, &mut all_type_definitions) {
            // Log the error but continue processing other files
            warn!(file = %file_path.display(), error = %e, "Failed to collect types from file, continuing");
        }
    }

    debug!(count = %all_type_definitions.len(), "Collected type definitions in Pass 1");

    // --- 2. Find Hyperprocess Impl Block & Extract Metadata ---
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
                interface_name = impl_item
                    .self_ty
                    .as_ref()
                    .as_type_path()
                    .and_then(|tp| tp.path.segments.last().map(|seg| seg.ident.to_string()));

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
                            return Err(e.wrap_err(format!(
                                "Invalid interface name '{}' in hyperprocess impl block",
                                name
                            )));
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

    // --- PASS 2: Process signatures and collect used types ---
    let mut signature_structs = Vec::new(); // Stores WIT string for each signature record
    let mut global_used_types = HashSet::new(); // All custom WIT types encountered (kebab-case)

    debug!("Pass 2: Analyzing functions in hyperprocess impl block");
    for item in &impl_item.items {
        if let ImplItem::Fn(method) = item {
            let method_name = method.sig.ident.to_string();
            debug!(method_name = %method_name, "Examining method");

            let has_remote = method.attrs.iter().any(|a| a.path().is_ident("remote"));
            let has_local = method.attrs.iter().any(|a| a.path().is_ident("local"));
            let has_http = method.attrs.iter().any(|a| a.path().is_ident("http"));
            let has_init = method.attrs.iter().any(|a| a.path().is_ident("init"));
            let has_ws = method.attrs.iter().any(|a| a.path().is_ident("ws"));
            let has_ws_client = method.attrs.iter().any(|a| a.path().is_ident("ws_client"));
            let has_terminal = method.attrs.iter().any(|a| a.path().is_ident("terminal"));

            if has_remote || has_local || has_http || has_init || has_ws || has_ws_client || has_terminal {
                debug!(remote=%has_remote, local=%has_local, http=%has_http, init=%has_init, ws=%has_ws, ws_client=%has_ws_client, terminal=%has_terminal, "Method attributes found");
                // Validate original Rust function name
                validate_name(&method_name, "Function")?; // Error early if name invalid
                let func_kebab_name = to_kebab_case(&method_name);

                if has_init {
                    debug!(method_name = %method_name, "Found [init] function, skipping signature generation");
                    continue;
                }

                if has_ws {
                    debug!(method_name = %method_name, "Found [ws] function, skipping signature generation (websocket handlers are ignored by WIT generator)");
                    continue;
                }

                if has_ws_client {
                    debug!(method_name = %method_name, "Found [ws_client] function, skipping signature generation (websocket handlers are ignored by WIT generator)");
                    continue;
                }

                if has_terminal {
                    debug!(method_name = %method_name, "Found [terminal] function, skipping signature generation (terminal handlers are ignored by WIT generator)");
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
                         "Method '{}' in the #[hyperprocess] impl block is missing a required attribute ([remote], [local], [http], [init], [ws], [ws_client], or [terminal]). Only methods with these attributes should be included.",
                         method_name
                     ));
            }
        }
    }
    debug!(signature_count = %signature_structs.len(), initial_used_types = ?global_used_types, "Completed signature analysis");

    // --- 3. Build dependency graph and topologically sort types ---
    debug!("Building type dependency graph");

    // Build a dependency map: type -> types it depends on
    let mut type_dependencies: HashMap<String, Vec<String>> = HashMap::new();
    let mut needed_types = HashSet::new();
    let mut to_process: Vec<String> = global_used_types
        .iter()
        .filter(|ty| !is_wit_primitive_or_builtin(ty))
        .cloned()
        .collect();

    // First pass: collect all needed types and their dependencies
    while let Some(type_name) = to_process.pop() {
        if needed_types.contains(&type_name) {
            continue;
        }

        // Check if we have a definition for this type
        if let Some(wit_def) = all_type_definitions.get(&type_name) {
            needed_types.insert(type_name.clone());
            let mut deps = Vec::new();

            // Extract nested type dependencies from the WIT definition
            // Look for other custom types referenced in this definition
            for (other_type_name, _) in &all_type_definitions {
                if other_type_name != &type_name && wit_def.contains(other_type_name) {
                    deps.push(other_type_name.clone());
                    if !needed_types.contains(other_type_name)
                        && !to_process.contains(other_type_name)
                    {
                        to_process.push(other_type_name.clone());
                    }
                }
            }

            type_dependencies.insert(type_name.clone(), deps);
        }
    }

    // Topological sort using Kahn's algorithm
    debug!("Performing topological sort of type definitions");
    let mut sorted_types = Vec::new();
    let mut in_degree: HashMap<String, usize> = HashMap::new();

    // Initialize in-degrees
    for type_name in &needed_types {
        in_degree.insert(type_name.clone(), 0);
    }

    // Calculate in-degrees
    for deps in type_dependencies.values() {
        for dep in deps {
            if let Some(degree) = in_degree.get_mut(dep) {
                *degree += 1;
            }
        }
    }

    // Find all types with in-degree 0
    let mut queue: Vec<String> = in_degree
        .iter()
        .filter(|(_, &degree)| degree == 0)
        .map(|(name, _)| name.clone())
        .collect();

    // Process queue
    while let Some(type_name) = queue.pop() {
        sorted_types.push(type_name.clone());

        // Reduce in-degree of dependent types
        if let Some(deps) = type_dependencies.get(&type_name) {
            for dep in deps {
                if let Some(degree) = in_degree.get_mut(dep) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push(dep.clone());
                    }
                }
            }
        }
    }

    // Check for cycles
    if sorted_types.len() != needed_types.len() {
        let missing: Vec<String> = needed_types
            .iter()
            .filter(|t| !sorted_types.contains(t))
            .cloned()
            .collect();
        warn!(missing = ?missing, "Circular dependency detected in type definitions");
        // Add remaining types anyway (WIT might still work)
        for t in missing {
            sorted_types.push(t);
        }
    }

    debug!(sorted_count = %sorted_types.len(), "Completed topological sort");

    // --- 4. Verify All Used Types Have Definitions ---
    debug!(final_used_types = ?global_used_types, available_definitions = ?all_type_definitions.keys(), "Starting final verification");
    let mut undefined_types = Vec::new();
    for used_type_name in &global_used_types {
        if !is_wit_primitive_or_builtin(used_type_name)
            && !all_type_definitions.contains_key(used_type_name)
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
    // Use topologically sorted types to ensure definitions come before uses
    let mut relevant_defs: Vec<String> = Vec::new();
    for type_name in &sorted_types {
        if let Some(def) = all_type_definitions.get(type_name) {
            relevant_defs.push(def.clone());
        }
    }
    // No need to sort again - already in topological order
    signature_structs.sort(); // Sort signature records for consistency

    if signature_structs.is_empty() && relevant_defs.is_empty() {
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
        if !relevant_defs.is_empty() {
            content.push('\n'); // Separator
            debug!(count=%relevant_defs.len(), "Adding type definitions to interface");
            content.push_str(&relevant_defs.join("\n\n"));
            content.push('\n');
        }

        // Add signature structs
        if !signature_structs.is_empty() {
            content.push('\n'); // Separator
            debug!(count=%signature_structs.len(), "Adding signature structs to interface");
            content.push_str(&signature_structs.join("\n\n"));
        }

        // Wrap in interface block
        let interface_wit_ident = to_wit_ident(kebab_name);
        let final_content = format!(
            "interface {} {{\n{}\n}}\n",
            interface_wit_ident,
            content.trim()
        ); // Trim any trailing whitespace
        debug!(interface_name = %interface_name.as_ref().unwrap(), signature_count = %signature_structs.len(), type_def_count = %relevant_defs.len(), "Generated interface content");

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
                    let import_wit_ident = to_wit_ident(&interface);
                    new_imports.push(format!("    import {};", import_wit_ident));
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
        info!(
            "No WIT interfaces generated and no target WIT worlds identified across all projects."
        );
        return Ok((processed_projects, interfaces)); // Return empty interfaces list
    } else if new_imports.is_empty() {
        info!(
            "No new WIT interfaces generated, but target WIT world(s) identified: {:?}",
            wit_worlds
        );
        // Proceed to rewrite world files even without new imports, as existing ones might need updates/creation.
    }

    // Update or create WIT world files
    debug!("Processing WIT world files for: {:?}", wit_worlds);
    let mut updated_world = false; // Track if any world file was written/updated

    rewrite_wit(
        api_dir,
        &new_imports,
        &mut wit_worlds.clone(),
        &mut updated_world,
    )?; // Pass a clone as rewrite_wit might modify it

    // If no world file was updated/created yet AND we have imports, create a default one.
    if !updated_world && !new_imports.is_empty() {
        // Define default world name
        let default_world = "async-app-template-dot-os-v0";
        warn!(default_world = %default_world, "No existing world definitions found or created for collected imports, creating default world file");

        // Determine include based on world name
        let include_line = if default_world.starts_with("types-") {
            "include lib;"
        } else {
            "include process-v1;"
        };

        let mut includes = HashSet::new();
        includes.insert(include_line.to_string());

        // Generate content using the helper function
        let world_content =
            generate_wit_file(default_world, &new_imports, &Vec::new(), &mut includes)?;

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
    }

    if !updated_world {
        info!("No world files were updated or created (either no imports needed adding, target worlds already existed/updated, or no default was needed).");
    }

    info!("WIT file generation process completed.");
    Ok((processed_projects, interfaces)) // Return list of successfully processed projects and generated interfaces
}
