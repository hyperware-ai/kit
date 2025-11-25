use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use color_eyre::{
    eyre::{bail, eyre, WrapErr},
    Result,
};
use tracing::{debug, info, instrument, warn};

use toml::Value;
use walkdir::WalkDir;

// Convert kebab-case to snake_case
pub fn to_snake_case(s: &str) -> String {
    s.replace('-', "_")
}

// Convert kebab-case to PascalCase
pub fn to_pascal_case(s: &str) -> String {
    let parts = s.split('-');
    let mut result = String::new();

    for part in parts {
        if !part.is_empty() {
            let mut chars = part.chars();
            if let Some(first_char) = chars.next() {
                result.push(first_char.to_uppercase().next().unwrap());
                result.extend(chars);
            }
        }
    }

    result
}

// Find the world name in the world WIT file, prioritizing types-prefixed worlds
#[instrument(level = "trace", skip_all)]
fn find_world_names(api_dir: &Path) -> Result<Vec<String>> {
    debug!(dir = ?api_dir, "Looking for world names...");
    let mut world_names = Vec::new();

    // Look for world definition files
    for entry in WalkDir::new(api_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "wit") {
            if let Ok(content) = fs::read_to_string(path) {
                if content.contains("world ") {
                    debug!(file = %path.display(), "Analyzing potential world definition file");

                    // Extract the world name
                    let lines: Vec<&str> = content.lines().collect();

                    if let Some(world_line) =
                        lines.iter().find(|line| line.trim().starts_with("world "))
                    {
                        debug!(line = %world_line, "Found world line");

                        if let Some(world_name) = world_line.trim().split_whitespace().nth(1) {
                            let clean_name = world_name.trim_end_matches(" {");
                            debug!(name = %clean_name, "Extracted potential world name");

                            // Check if this is a types-prefixed world
                            if clean_name.starts_with("types-") {
                                world_names.push(clean_name.to_string());
                                debug!(name = %clean_name, "Found types-prefixed world");
                            }
                        }
                    }
                }
            }
        }
    }

    if world_names.is_empty() {
        bail!("No world name found in any WIT file. Cannot generate caller-utils without a world name.")
    }
    Ok(world_names)
}

// Convert WIT type to Rust type - IMPROVED with more Rust primitives
fn wit_type_to_rust(wit_type: &str) -> String {
    match wit_type {
        // Integer types
        "s8" => "i8".to_string(),
        "u8" => "u8".to_string(),
        "s16" => "i16".to_string(),
        "u16" => "u16".to_string(),
        "s32" => "i32".to_string(),
        "u32" => "u32".to_string(),
        "s64" => "i64".to_string(),
        "u64" => "u64".to_string(),
        // Floating point types
        "f32" => "f32".to_string(),
        "f64" => "f64".to_string(),
        // Other primitive types
        "string" => "String".to_string(),
        "str" => "&str".to_string(),
        "char" => "char".to_string(),
        "bool" => "bool".to_string(),
        "_" => "()".to_string(),
        // Special types
        "address" => "WitAddress".to_string(),
        // Collection types with generics
        t if t.starts_with("list<") => {
            let inner_type = &t[5..t.len() - 1];
            format!("Vec<{}>", wit_type_to_rust(inner_type))
        }
        t if t.starts_with("option<") => {
            let inner_type = &t[7..t.len() - 1];
            format!("Option<{}>", wit_type_to_rust(inner_type))
        }
        t if t.starts_with("result<") => {
            let inner_part = &t[7..t.len() - 1];
            if let Some(comma_pos) = inner_part.find(',') {
                let ok_type = &inner_part[..comma_pos].trim();
                let err_type = &inner_part[comma_pos + 1..].trim();
                format!(
                    "Result<{}, {}>",
                    wit_type_to_rust(ok_type),
                    wit_type_to_rust(err_type)
                )
            } else {
                format!("Result<{}, ()>", wit_type_to_rust(inner_part))
            }
        }
        t if t.starts_with("tuple<") => {
            let inner_types = &t[6..t.len() - 1];
            let rust_types: Vec<String> = inner_types
                .split(", ")
                .map(|t| wit_type_to_rust(t))
                .collect();
            format!("({})", rust_types.join(", "))
        }
        // Custom types (in kebab-case) need to be converted to PascalCase
        _ => to_pascal_case(wit_type).to_string(),
    }
}

// Structure to represent a field in a WIT signature struct
#[derive(Debug)]
struct SignatureField {
    name: String,
    wit_type: String,
}

/// Parse a tuple type string like "tuple<u64, bool>" into its element types
fn parse_tuple_types(tuple_type: &str) -> Vec<String> {
    if !tuple_type.starts_with("tuple<") || !tuple_type.ends_with(">") {
        return vec![];
    }
    let inner = &tuple_type[6..tuple_type.len() - 1];
    if inner.is_empty() {
        return vec![];
    }
    // Handle nested generics by tracking angle bracket depth
    let mut types = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for c in inner.chars() {
        match c {
            '<' => {
                depth += 1;
                current.push(c);
            }
            '>' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                types.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(c);
            }
        }
    }
    if !current.trim().is_empty() {
        types.push(current.trim().to_string());
    }
    types
}

// Structure to represent a WIT signature struct
#[derive(Debug)]
struct SignatureStruct {
    function_name: String,
    attr_type: String,
    fields: Vec<SignatureField>,
}

// Find all interface imports in the world WIT file
#[instrument(level = "trace", skip_all)]
fn find_interfaces_in_world(api_dir: &Path) -> Result<Vec<String>> {
    debug!(dir = ?api_dir, "Finding interface imports in world definitions");
    let mut interfaces = Vec::new();

    // Find world definition files
    for entry in WalkDir::new(api_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "wit") {
            if let Ok(content) = fs::read_to_string(path) {
                if content.contains("world ") {
                    debug!(file = %path.display(), "Analyzing world definition file for imports");

                    // Extract import statements
                    for line in content.lines() {
                        let line = line.trim();
                        if line.starts_with("import ") && line.ends_with(";") {
                            let interface = line
                                .trim_start_matches("import ")
                                .trim_end_matches(";")
                                .trim();

                            interfaces.push(interface.to_string());
                            debug!(interface = %interface, "Found interface import");
                        }
                    }
                }
            }
        }
    }
    debug!(count = interfaces.len(), interfaces = ?interfaces, "Found interface imports");
    Ok(interfaces)
}

// Parse WIT file to extract function signatures and type definitions
#[instrument(level = "trace", skip_all)]
fn parse_wit_file(file_path: &Path) -> Result<(Vec<SignatureStruct>, Vec<String>)> {
    debug!(file = %file_path.display(), "Parsing WIT file");

    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read WIT file: {}", file_path.display()))?;

    let mut signatures = Vec::new();
    let mut type_names = Vec::new();

    // Simple parser for WIT files to extract record definitions and types
    let lines: Vec<_> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Look for record definitions that aren't signature structs
        if line.starts_with("record ") && !line.contains("-signature-") {
            let record_name = line
                .trim_start_matches("record ")
                .trim_end_matches(" {")
                .trim();
            debug!(name = %record_name, "Found type definition (record)");
            type_names.push(record_name.to_string());
        }
        // Look for variant definitions (enums)
        else if line.starts_with("variant ") {
            let variant_name = line
                .trim_start_matches("variant ")
                .trim_end_matches(" {")
                .trim();
            debug!(name = %variant_name, "Found type definition (variant)");
            type_names.push(variant_name.to_string());
        }
        // Look for signature record definitions
        else if line.starts_with("record ") && line.contains("-signature-") {
            let record_name = line
                .trim_start_matches("record ")
                .trim_end_matches(" {")
                .trim();
            debug!(name = %record_name, "Found signature record");

            // Extract function name and attribute type
            let parts: Vec<_> = record_name.split("-signature-").collect();
            if parts.len() != 2 {
                warn!(name = %record_name, "Unexpected signature record name format, skipping");
                i += 1;
                continue;
            }

            let function_name = parts[0].to_string();
            let attr_type = parts[1].to_string();
            debug!(function = %function_name, attr_type = %attr_type, "Extracted function name and type");

            // Parse fields
            let mut fields = Vec::new();
            i += 1;

            while i < lines.len() && !lines[i].trim().starts_with("}") {
                let field_line = lines[i].trim();

                // Skip comments and empty lines
                if field_line.starts_with("//") || field_line.is_empty() {
                    i += 1;
                    continue;
                }

                // Parse field definition
                let field_parts: Vec<_> = field_line.split(':').collect();
                if field_parts.len() == 2 {
                    let field_name = field_parts[0].trim().to_string();
                    let field_type = field_parts[1].trim().trim_end_matches(',').to_string();

                    debug!(name = %field_name, wit_type = %field_type, "Found field");
                    fields.push(SignatureField {
                        name: field_name,
                        wit_type: field_type,
                    });
                }

                i += 1;
            }

            signatures.push(SignatureStruct {
                function_name,
                attr_type,
                fields,
            });
        }

        i += 1;
    }

    debug!(
        file = %file_path.display(),
        signatures = signatures.len(),
        types = type_names.len(),
        "Finished parsing WIT file"
    );
    Ok((signatures, type_names))
}

// Generate a Rust async function from a signature struct
fn generate_async_function(signature: &SignatureStruct) -> Option<String> {
    // Convert function name from kebab-case to snake_case
    let snake_function_name = to_snake_case(&signature.function_name);

    // Get pascal case version for the JSON request format
    let pascal_function_name = to_pascal_case(&signature.function_name);

    // Function full name with attribute type
    let full_function_name = format!("{}_{}_rpc", snake_function_name, signature.attr_type);
    debug!(name = %full_function_name, "Generating function stub");

    // Extract parameters and return type
    let mut params = Vec::new();
    let mut param_names = Vec::new();
    let mut return_type = "()".to_string();
    let mut target_param = "";

    for field in &signature.fields {
        let rust_type = wit_type_to_rust(&field.wit_type);
        debug!(field = %field.name, wit_type = %field.wit_type, rust_type = %rust_type, "Processing field");

        if field.name == "target" {
            if field.wit_type == "string" {
                target_param = "&str";
            } else {
                // Use hyperware_process_lib::Address instead of WitAddress
                target_param = "&Address";
            }
        } else if field.name == "returning" {
            return_type = rust_type;
            debug!(return_type = %return_type, "Identified return type");
        } else if field.name == "args" {
            // Parse the args tuple to extract individual parameter types
            let tuple_types = parse_tuple_types(&field.wit_type);
            for (i, wit_type) in tuple_types.iter().enumerate() {
                let param_name = format!("arg{}", i);
                let param_rust_type = wit_type_to_rust(wit_type);
                params.push(format!("{}: {}", param_name, param_rust_type));
                param_names.push(param_name);
                debug!(param_name = param_names.last().unwrap(), wit_type = %wit_type, "Added tuple parameter");
            }
        } else {
            // Legacy support: handle individual parameter fields (for backwards compatibility)
            let field_name_snake = to_snake_case(&field.name);
            params.push(format!("{}: {}", field_name_snake, rust_type));
            param_names.push(field_name_snake);
            debug!(
                param_name = param_names.last().unwrap(),
                "Added parameter (legacy)"
            );
        }
    }

    // First parameter is always target
    let all_params = if target_param.is_empty() {
        warn!(
            "No 'target' parameter found in signature for {}",
            full_function_name
        );
        params.join(", ")
    } else {
        format!(
            "target: {}{}",
            target_param,
            if params.is_empty() { "" } else { ", " }
        ) + &params.join(", ")
    };

    // Wrap the return type in a Result<_, AppSendError>
    let wrapped_return_type = format!("Result<{}, AppSendError>", return_type);

    // For HTTP endpoints, generate commented-out implementation
    if signature.attr_type == "http" {
        return None;
    }

    // Format JSON parameters correctly
    let json_params = if param_names.is_empty() {
        // No parameters case
        debug!("Generating JSON with no parameters");
        format!("json!({{\"{}\" : null}})", pascal_function_name)
    } else if param_names.len() == 1 {
        // Single parameter case
        debug!(param = %param_names[0], "Generating JSON with single parameter");
        format!(
            "json!({{\"{}\": {}}})",
            pascal_function_name, param_names[0]
        )
    } else {
        // Multiple parameters case - use tuple format
        debug!(params = ?param_names, "Generating JSON with multiple parameters (tuple)");
        format!(
            "json!({{\"{}\": ({})}})",
            pascal_function_name,
            param_names.join(", ")
        )
    };

    // Generate function with implementation using send
    debug!("Generating standard RPC stub implementation");
    Some(format!(
        "/// Generated stub for `{}` {} RPC call\npub async fn {}({}) -> {} {{\n    let body = {};\n    let body = serde_json::to_vec(&body).unwrap();\n    let request = Request::to(target)\n        .body(body);\n    send::<{}>(request).await\n}}",
        signature.function_name,
        signature.attr_type,
        full_function_name,
        all_params,
        wrapped_return_type,
        json_params,
        return_type
    ))
}

// Create the caller-utils crate with a single lib.rs file
#[instrument(level = "trace", skip_all)]
fn create_caller_utils_crate(api_dir: &Path, base_dir: &Path) -> Result<()> {
    // Extract package name from base directory
    let package_name = base_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| eyre!("Could not extract package name from base directory"))?;

    // Create crate name by prepending package name
    let crate_name = format!("{}-caller-utils", package_name);

    // Path to the new crate
    let caller_utils_dir = base_dir.join("target").join(&crate_name);
    debug!(
        path = %caller_utils_dir.display(),
        crate_name = %crate_name,
        "Creating caller-utils crate"
    );

    // Create directories
    fs::create_dir_all(&caller_utils_dir)?;
    fs::create_dir_all(caller_utils_dir.join("src"))?;
    debug!("Created project directory structure");

    // Get hyperware_process_lib dependency from the process's Cargo.toml
    let hyperware_dep = get_hyperware_process_lib_dependency(base_dir)?;
    debug!("Got hyperware_process_lib dependency: {}", hyperware_dep);

    // Create Cargo.toml with updated dependencies
    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
anyhow = "1.0"
process_macros = "0.1.0"
futures-util = "0.3"
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
hyperware_process_lib = {}
once_cell = "1.20.2"
futures = "0.3"
uuid = {{ version = "1.0" }}
wit-bindgen = "0.41.0"

[lib]
crate-type = ["cdylib", "lib"]
"#,
        crate_name.replace("-", "_"),
        hyperware_dep
    );

    fs::write(caller_utils_dir.join("Cargo.toml"), cargo_toml)
        .with_context(|| format!("Failed to write {} Cargo.toml", crate_name))?;

    debug!("Created Cargo.toml for {}", crate_name);

    // Get the world name (preferably the types- version)
    let world_names = find_world_names(api_dir)?;
    debug!("Using world names for code generation: {:?}", world_names);
    let world_name = if world_names.len() == 0 {
        ""
    } else if world_names.len() == 1 {
        &world_names[0]
    } else {
        let path = api_dir.join("types.wit");
        let mut content = "world types {\n".to_string();
        for world_name in world_names {
            content.push_str(&format!("    include {world_name};\n"));
        }
        content.push_str("}\n");
        fs::write(&path, &content)?;
        "types"
    };

    // Get all interfaces from the world file
    let interface_imports = find_interfaces_in_world(api_dir)?;

    // Store all types from each interface
    let mut interface_types: HashMap<String, Vec<String>> = HashMap::new();

    // Find all WIT files in the api directory to generate stubs
    let mut wit_files = Vec::new();
    for entry in WalkDir::new(api_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "wit") {
            // Exclude world definition files
            if let Ok(content) = fs::read_to_string(path) {
                if !content.contains("world ") {
                    debug!(file = %path.display(), "Adding WIT file for parsing");
                    wit_files.push(path.to_path_buf());
                } else {
                    debug!(file = %path.display(), "Skipping world definition WIT file");
                }
            }
        }
    }

    debug!(
        count = wit_files.len(),
        "Found WIT interface files for stub generation"
    );

    // Generate content for each module and collect types
    let mut module_contents = HashMap::<String, String>::new();

    for wit_file in &wit_files {
        // Extract the interface name from the file name
        let interface_name = wit_file.file_stem().unwrap().to_string_lossy();
        let snake_interface_name = to_snake_case(&interface_name);

        debug!(
            interface = %interface_name, module = %snake_interface_name, file = %wit_file.display(),
            "Processing interface"
        );

        // Parse the WIT file to extract signature structs and types
        match parse_wit_file(wit_file) {
            Ok((signatures, types)) => {
                // Store types for this interface
                interface_types.insert(interface_name.to_string(), types);

                if signatures.is_empty() {
                    debug!(file = %wit_file.display(), "No signature records found, skipping module generation for this file.");
                    continue;
                }

                // Generate module content
                let mut mod_content = String::new();

                // Add function implementations
                for signature in &signatures {
                    if let Some(function_impl) = generate_async_function(signature) {
                        mod_content.push_str(&function_impl);
                        mod_content.push_str("\n\n");
                    }
                }

                // Store the module content
                module_contents.insert(snake_interface_name.clone(), mod_content);

                debug!(
                    interface = %interface_name, module = %snake_interface_name.as_str(), count = signatures.len(),
                    "Generated module content"
                );
            }
            Err(e) => {
                warn!(file = %wit_file.display(), error = %e, "Error parsing WIT file, skipping");
            }
        }
    }

    // Create import statements for each interface using "hyperware::process::{interface_name}::*"
    // Use a HashSet to track which interfaces we've already processed to avoid duplicates
    let mut processed_interfaces = std::collections::HashSet::new();
    let mut interface_use_statements = Vec::new();

    for interface_name in &interface_imports {
        // Convert to snake case for module name
        let snake_interface_name = to_snake_case(interface_name);

        // Only add the import if we haven't processed this interface yet
        if processed_interfaces.insert(snake_interface_name.clone()) {
            // Create wildcard import for this interface
            interface_use_statements.push(format!(
                "pub use crate::hyperware::process::{}::*;",
                snake_interface_name
            ));
        }
    }

    // Create single lib.rs with all modules inline
    let mut lib_rs = String::new();

    lib_rs.push_str("wit_bindgen::generate!({\n");
    lib_rs.push_str("    path: \"target/wit\",\n");
    lib_rs.push_str(&format!("    world: \"{}\",\n", world_name));
    lib_rs.push_str("    generate_unused_types: true,\n");
    lib_rs.push_str("    additional_derives: [serde::Deserialize, serde::Serialize, process_macros::SerdeJsonInto],\n");
    lib_rs.push_str("});\n\n");

    lib_rs.push_str("/// Generated caller utilities for RPC function stubs\n\n");

    // Add global imports
    lib_rs.push_str("pub use hyperware_process_lib::hyperapp::AppSendError;\n");
    lib_rs.push_str("pub use hyperware_process_lib::hyperapp::send;\n");
    lib_rs.push_str("pub use hyperware_process_lib::{Address, Request};\n");
    lib_rs.push_str("use serde_json::json;\n\n");

    // Add interface use statements
    if !interface_use_statements.is_empty() {
        lib_rs.push_str("// Import types from each interface\n");
        for use_stmt in interface_use_statements {
            lib_rs.push_str(&format!("{}\n", use_stmt));
        }
        lib_rs.push_str("\n");
    }

    // Add all modules with their content
    for (module_name, module_content) in module_contents {
        lib_rs.push_str(&format!(
            "/// Generated RPC stubs for the {} interface\n",
            module_name
        ));
        lib_rs.push_str(&format!("pub mod {} {{\n", module_name));
        lib_rs.push_str("    use crate::*;\n\n");
        lib_rs.push_str(&format!("    {}\n", module_content.replace("\n", "\n    ")));
        lib_rs.push_str("}\n\n");
    }

    // Write lib.rs
    let lib_rs_path = caller_utils_dir.join("src").join("lib.rs");
    debug!("Writing generated code to {}", lib_rs_path.display());

    fs::write(&lib_rs_path, lib_rs)
        .with_context(|| format!("Failed to write lib.rs: {}", lib_rs_path.display()))?;

    // Create target/wit directory and copy all WIT files
    let target_wit_dir = caller_utils_dir.join("target").join("wit");
    debug!("Creating directory: {}", target_wit_dir.display());

    // Remove the directory if it exists to ensure clean state
    if target_wit_dir.exists() {
        debug!("Removing existing target/wit directory");
        fs::remove_dir_all(&target_wit_dir)?;
    }

    fs::create_dir_all(&target_wit_dir)?;

    // Copy all WIT files to target/wit
    for entry in WalkDir::new(api_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "wit") {
            let file_name = path.file_name().unwrap();
            let target_path = target_wit_dir.join(file_name);
            fs::copy(path, &target_path).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    path.display(),
                    target_path.display()
                )
            })?;
            debug!(
                "Copied {} to target/wit directory",
                file_name.to_string_lossy()
            );
        }
    }

    Ok(())
}

// Format a TOML dependency value into an inline table string
fn format_toml_dependency(dep: &Value) -> Option<String> {
    match dep {
        Value::Table(table) => {
            let fields = [
                ("git", None),
                ("rev", None),
                ("branch", None),
                ("tag", None),
                ("version", None),
                ("path", None),
                (
                    "features",
                    Some(|v: &Value| -> Option<String> {
                        Some(
                            v.as_array()?
                                .iter()
                                .filter_map(|f| f.as_str())
                                .map(|f| format!("\"{}\"", f))
                                .collect::<Vec<_>>()
                                .join(", "),
                        )
                    }),
                ),
            ];

            let parts: Vec<String> = fields
                .iter()
                .filter_map(|(key, formatter)| {
                    let value = table.get(*key)?;
                    if let Some(format_fn) = formatter {
                        Some(format!("{} = [{}]", key, format_fn(value)?))
                    } else {
                        Some(format!("{} = \"{}\"", key, value.as_str()?))
                    }
                })
                .collect();

            Some(format!("{{ {} }}", parts.join(", ")))
        }
        Value::String(s) => Some(format!("\"{}\"", s)),
        _ => None,
    }
}

// Read and parse a Cargo.toml file
fn read_cargo_toml(path: &Path) -> Result<Value> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read Cargo.toml: {}", path.display()))?;
    content
        .parse()
        .with_context(|| format!("Failed to parse Cargo.toml: {}", path.display()))
}

// Get hyperware_process_lib dependency from the process Cargo.toml files
#[instrument(level = "trace", skip_all)]
fn get_hyperware_process_lib_dependency(base_dir: &Path) -> Result<String> {
    const DEFAULT_DEP: &str =
        r#"{ git = "https://github.com/hyperware-ai/hyperprocess-macro", rev = "4c944b2" }"#;

    // Read workspace members
    let workspace_toml = read_cargo_toml(&base_dir.join("Cargo.toml"))?;
    let members = workspace_toml
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .ok_or_else(|| eyre!("No workspace.members found in Cargo.toml"))?;

    // Collect hyperware_process_lib dependencies from all process members
    let mut found_deps = HashMap::new();

    for member in members.iter().filter_map(|m| m.as_str()) {
        // Skip generated directories
        if member.starts_with("target/") {
            continue;
        }

        let member_cargo_path = base_dir.join(member).join("Cargo.toml");
        if !member_cargo_path.exists() {
            debug!(
                "Member Cargo.toml not found: {}",
                member_cargo_path.display()
            );
            continue;
        }

        let member_toml = read_cargo_toml(&member_cargo_path)?;

        if let Some(dep) = member_toml
            .get("dependencies")
            .and_then(|d| d.get("hyperware_process_lib"))
            .and_then(format_toml_dependency)
        {
            debug!("Found hyperware_process_lib in {}: {}", member, dep);
            found_deps.insert(member.to_string(), dep);
        }
    }

    // Handle results
    match found_deps.len() {
        0 => {
            warn!("No hyperware_process_lib dependencies found in any process, using default");
            Ok(DEFAULT_DEP.to_string())
        }
        1 => {
            let dep = found_deps.values().next().unwrap();
            info!("Using hyperware_process_lib dependency: {}", dep);
            Ok(dep.clone())
        }
        _ => {
            // Ensure all dependencies match
            let mut deps_iter = found_deps.values();
            let first_dep = deps_iter.next().unwrap();

            for dep in deps_iter {
                if dep != first_dep {
                    let (first_process, _) =
                        found_deps.iter().find(|(_, d)| *d == first_dep).unwrap();
                    let (conflict_process, _) = found_deps.iter().find(|(_, d)| *d == dep).unwrap();
                    bail!(
                        "Conflicting hyperware_process_lib versions found:\n  Process '{}': {}\n  Process '{}': {}\nAll processes must use the same version.",
                        first_process, first_dep, conflict_process, dep
                    );
                }
            }

            info!("Using hyperware_process_lib dependency: {}", first_dep);
            Ok(first_dep.clone())
        }
    }
}

// Update workspace Cargo.toml to include the caller-utils crate
#[instrument(level = "trace", skip_all)]
fn update_workspace_cargo_toml(base_dir: &Path, crate_name: &str) -> Result<()> {
    let workspace_cargo_toml = base_dir.join("Cargo.toml");
    debug!(
        path = %workspace_cargo_toml.display(),
        "Updating workspace Cargo.toml"
    );

    if !workspace_cargo_toml.exists() {
        warn!(
            path = %workspace_cargo_toml.display(),
            "Workspace Cargo.toml not found, skipping update."
        );
        return Ok(());
    }

    let content = fs::read_to_string(&workspace_cargo_toml).with_context(|| {
        format!(
            "Failed to read workspace Cargo.toml: {}",
            workspace_cargo_toml.display()
        )
    })?;

    // Parse the TOML content
    let mut parsed_toml: Value = content
        .parse()
        .with_context(|| "Failed to parse workspace Cargo.toml")?;

    // Check if there's a workspace section
    if let Some(workspace) = parsed_toml.get_mut("workspace") {
        if let Some(members) = workspace.get_mut("members") {
            if let Some(members_array) = members.as_array_mut() {
                // Check if caller-utils is already in the members list
                // Using a `?` forces cargo to interpret it as optional, which allows building from scratch (i.e. before caller-utils has been generated)
                let crate_name_without_s = crate_name.trim_end_matches('s');
                let target_path = format!("target/{}?", crate_name_without_s);
                let caller_utils_exists = members_array
                    .iter()
                    .any(|m| m.as_str().map_or(false, |s| s == target_path));

                if !caller_utils_exists {
                    members_array.push(Value::String(target_path.clone()));

                    // Write back the updated TOML
                    let updated_content = toml::to_string_pretty(&parsed_toml)
                        .with_context(|| "Failed to serialize updated workspace Cargo.toml")?;

                    fs::write(&workspace_cargo_toml, updated_content).with_context(|| {
                        format!(
                            "Failed to write updated workspace Cargo.toml: {}",
                            workspace_cargo_toml.display()
                        )
                    })?;

                    debug!("Successfully updated workspace Cargo.toml");
                } else {
                    debug!(
                        "Workspace Cargo.toml already up-to-date regarding {} member.",
                        target_path
                    );
                }
            }
        }
    }

    Ok(())
}

// Add caller-utils as a dependency to hyperware:process crates
#[instrument(level = "trace", skip_all)]
pub fn add_caller_utils_to_projects(projects: &[PathBuf], base_dir: &Path) -> Result<()> {
    // Extract package name from base directory
    let package_name = base_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| eyre!("Could not extract package name from base directory"))?;

    // Create crate name by prepending package name
    let crate_name = format!("{}-caller-utils", package_name);
    let crate_name_underscore = crate_name.replace("-", "_");
    for project_path in projects {
        let cargo_toml_path = project_path.join("Cargo.toml");
        debug!(
            project = ?project_path.file_name().unwrap_or_default(),
            path = %cargo_toml_path.display(),
            "Processing project"
        );

        let content = fs::read_to_string(&cargo_toml_path).with_context(|| {
            format!(
                "Failed to read project Cargo.toml: {}",
                cargo_toml_path.display()
            )
        })?;

        let mut parsed_toml: Value = content.parse().with_context(|| {
            format!(
                "Failed to parse project Cargo.toml: {}",
                cargo_toml_path.display()
            )
        })?;

        // Add caller-utils to dependencies if not already present
        if let Some(dependencies) = parsed_toml.get_mut("dependencies") {
            if let Some(deps_table) = dependencies.as_table_mut() {
                if !deps_table.contains_key(&crate_name_underscore) {
                    deps_table.insert(
                        crate_name_underscore.clone(),
                        Value::Table({
                            let mut t = toml::map::Map::new();
                            t.insert(
                                "path".to_string(),
                                Value::String(format!("../target/{}", crate_name)),
                            );
                            t.insert("optional".to_string(), Value::Boolean(true));
                            t
                        }),
                    );

                    debug!(project = ?project_path.file_name().unwrap_or_default(), "Successfully added {} dependency", crate_name_underscore);
                } else {
                    debug!(project = ?project_path.file_name().unwrap_or_default(), "{} dependency already exists", crate_name_underscore);
                }
            }
        }

        // Add or update the features section to include caller-utils feature
        if !parsed_toml.as_table().unwrap().contains_key("features") {
            parsed_toml
                .as_table_mut()
                .unwrap()
                .insert("features".to_string(), Value::Table(toml::map::Map::new()));
        }

        if let Some(features) = parsed_toml.get_mut("features") {
            if let Some(features_table) = features.as_table_mut() {
                // Add caller-utils feature that enables the package-specific caller-utils dependency
                if !features_table.contains_key("caller-utils") {
                    features_table.insert(
                        "caller-utils".to_string(),
                        Value::Array(vec![Value::String(crate_name_underscore.clone())]),
                    );
                    debug!(project = ?project_path.file_name().unwrap_or_default(), "Added caller-utils feature");
                } else {
                    // Update existing caller-utils feature if it doesn't include our dependency
                    if let Some(caller_utils_feature) = features_table.get_mut("caller-utils") {
                        if let Some(feature_array) = caller_utils_feature.as_array_mut() {
                            let dep_exists = feature_array
                                .iter()
                                .any(|v| v.as_str().map_or(false, |s| s == crate_name_underscore));
                            if !dep_exists {
                                feature_array.push(Value::String(crate_name_underscore.clone()));
                                debug!(project = ?project_path.file_name().unwrap_or_default(), "Updated caller-utils feature to include {}", crate_name_underscore);
                            }
                        }
                    }
                }
            }
        }

        // Write back the updated TOML
        let updated_content = toml::to_string_pretty(&parsed_toml).with_context(|| {
            format!(
                "Failed to serialize updated project Cargo.toml: {}",
                cargo_toml_path.display()
            )
        })?;

        fs::write(&cargo_toml_path, updated_content).with_context(|| {
            format!(
                "Failed to write updated project Cargo.toml: {}",
                cargo_toml_path.display()
            )
        })?;
    }

    Ok(())
}

// Create caller-utils crate and integrate with the workspace
#[instrument(level = "trace", skip_all)]
pub fn create_caller_utils(base_dir: &Path, api_dir: &Path) -> Result<()> {
    // Extract package name from base directory
    let package_name = base_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| eyre!("Could not extract package name from base directory"))?;

    // Create crate name by prepending package name
    let crate_name = format!("{}-caller-utils", package_name);

    // Step 1: Create the caller-utils crate
    create_caller_utils_crate(api_dir, base_dir)?;

    // Step 2: Update workspace Cargo.toml
    update_workspace_cargo_toml(base_dir, &crate_name)?;

    info!("Successfully created {} and copied the imports", crate_name);
    Ok(())
}
