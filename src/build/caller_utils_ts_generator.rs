use std::fs;
use std::path::Path;

use color_eyre::{eyre::WrapErr, Result};
use tracing::{debug, info, instrument, warn};

use walkdir::WalkDir;

// Convert kebab-case to camelCase
pub fn to_camel_case(s: &str) -> String {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.is_empty() {
        return String::new();
    }

    let mut result = parts[0].to_string();
    for part in &parts[1..] {
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

// Convert WIT type to TypeScript type
fn wit_type_to_typescript(wit_type: &str) -> String {
    match wit_type {
        // Integer types - all become number in TypeScript
        "s8" | "u8" | "s16" | "u16" | "s32" | "u32" | "s64" | "u64" => "number".to_string(),
        // Floating point types
        "f32" | "f64" => "number".to_string(),
        // Other primitive types
        "string" => "string".to_string(),
        "bool" => "boolean".to_string(),
        "_" => "void".to_string(),
        // Special types
        "address" => "string".to_string(), // Address would be a string in TypeScript
        // Collection types with generics
        t if t.starts_with("list<") => {
            let inner_type = &t[5..t.len() - 1];
            // Special case for list<u8> which becomes number[]
            if inner_type == "u8" {
                "number[]".to_string()
            } else {
                format!("{}[]", wit_type_to_typescript(inner_type))
            }
        }
        t if t.starts_with("option<") => {
            let inner_type = &t[7..t.len() - 1];
            format!("{} | null", wit_type_to_typescript(inner_type))
        }
        t if t.starts_with("result<") => {
            let inner_part = &t[7..t.len() - 1];
            if let Some(comma_pos) = inner_part.find(',') {
                let ok_type = &inner_part[..comma_pos].trim();
                let err_type = &inner_part[comma_pos + 1..].trim();
                format!(
                    "{{ Ok: {} }} | {{ Err: {} }}",
                    wit_type_to_typescript(ok_type),
                    wit_type_to_typescript(err_type)
                )
            } else {
                format!(
                    "{{ Ok: {} }} | {{ Err: void }}",
                    wit_type_to_typescript(inner_part)
                )
            }
        }
        t if t.starts_with("tuple<") => {
            let inner_types = &t[6..t.len() - 1];
            let ts_types: Vec<String> = inner_types
                .split(", ")
                .map(|t| wit_type_to_typescript(t))
                .collect();
            format!("[{}]", ts_types.join(", "))
        }
        // Custom types (in kebab-case) need to be converted to PascalCase
        _ => to_pascal_case(wit_type).to_string(),
    }
}

// Extract the inner type from a Result type for function returns
fn extract_result_ok_type(wit_type: &str) -> Option<String> {
    if wit_type.starts_with("result<") {
        let inner_part = &wit_type[7..wit_type.len() - 1];
        if let Some(comma_pos) = inner_part.find(',') {
            let ok_type = inner_part[..comma_pos].trim();
            Some(wit_type_to_typescript(ok_type))
        } else {
            // Result with no error type
            Some(wit_type_to_typescript(inner_part))
        }
    } else {
        None
    }
}

// Structure to represent a field in a WIT signature struct
#[derive(Debug)]
struct SignatureField {
    name: String,
    wit_type: String,
}

// Structure to represent a WIT signature struct
#[derive(Debug)]
struct SignatureStruct {
    function_name: String,
    attr_type: String,
    fields: Vec<SignatureField>,
}

// Parse WIT file to extract function signatures
#[instrument(level = "trace", skip_all)]
fn parse_wit_file(file_path: &Path) -> Result<Vec<SignatureStruct>> {
    debug!(file = %file_path.display(), "Parsing WIT file");

    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read WIT file: {}", file_path.display()))?;

    let mut signatures = Vec::new();

    // Simple parser for WIT files to extract record definitions
    let lines: Vec<_> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Look for signature record definitions
        if line.starts_with("record ") && line.contains("-signature-") {
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
        "Finished parsing WIT file"
    );
    Ok(signatures)
}

// Generate TypeScript interface and function from a signature struct
fn generate_typescript_function(signature: &SignatureStruct) -> (String, String, String) {
    // Convert function name from kebab-case to camelCase
    let camel_function_name = to_camel_case(&signature.function_name);
    let pascal_function_name = to_pascal_case(&signature.function_name);

    debug!(name = %camel_function_name, "Generating TypeScript function");

    // Extract parameters and return type
    let mut params = Vec::new();
    let mut param_names = Vec::new();
    let mut param_types = Vec::new();
    let mut full_return_type = "void".to_string();
    let mut unwrapped_return_type = "void".to_string();

    for field in &signature.fields {
        let field_name_camel = to_camel_case(&field.name);
        let ts_type = wit_type_to_typescript(&field.wit_type);
        debug!(field = %field.name, wit_type = %field.wit_type, ts_type = %ts_type, "Processing field");

        if field.name == "target" {
            // Skip target field as it's handled internally
            continue;
        } else if field.name == "returning" {
            full_return_type = ts_type.clone();
            // Check if it's a Result type and extract the Ok type
            if let Some(ok_type) = extract_result_ok_type(&field.wit_type) {
                unwrapped_return_type = ok_type;
            } else {
                unwrapped_return_type = ts_type;
            }
            debug!(return_type = %unwrapped_return_type, "Identified return type");
        } else {
            params.push(format!("{}: {}", field_name_camel, ts_type));
            param_names.push(field_name_camel);
            param_types.push(ts_type);
        }
    }

    // Generate request interface
    let request_interface = if param_names.is_empty() {
        // No parameters case
        format!(
            "export interface {}Request {{\n  {}: {{}}\n}}",
            pascal_function_name, pascal_function_name
        )
    } else if param_names.len() == 1 {
        // Single parameter case
        format!(
            "export interface {}Request {{\n  {}: {}\n}}",
            pascal_function_name, pascal_function_name, param_types[0]
        )
    } else {
        // Multiple parameters case - use tuple format
        format!(
            "export interface {}Request {{\n  {}: [{}]\n}}",
            pascal_function_name,
            pascal_function_name,
            param_types.join(", ")
        )
    };

    // Generate response type alias (using the full Result type)
    let response_type = format!(
        "export type {}Response = {};",
        pascal_function_name, full_return_type
    );

    // Generate function implementation
    let function_params = params.join(", ");

    let data_construction = if param_names.is_empty() {
        format!(
            "  const data: {}Request = {{\n    {}: {{}},\n  }};",
            pascal_function_name, pascal_function_name
        )
    } else if param_names.len() == 1 {
        format!(
            "  const data: {}Request = {{\n    {}: {},\n  }};",
            pascal_function_name, pascal_function_name, param_names[0]
        )
    } else {
        format!(
            "  const data: {}Request = {{\n    {}: [{}],\n  }};",
            pascal_function_name,
            pascal_function_name,
            param_names.join(", ")
        )
    };

    // Function returns the unwrapped type since parseResultResponse extracts it
    let function_impl = format!(
        "/**\n * {}\n{} * @returns Promise with result\n * @throws ApiError if the request fails\n */\nexport async function {}({}): Promise<{}> {{\n{}\n\n  return await apiRequest<{}Request, {}>('{}', 'POST', data);\n}}",
        camel_function_name,
        params.iter().map(|p| format!(" * @param {}", p)).collect::<Vec<_>>().join("\n"),
        camel_function_name,
        function_params,
        unwrapped_return_type,  // Use unwrapped type as the function return
        data_construction,
        pascal_function_name,
        unwrapped_return_type,  // Pass unwrapped type to apiRequest, not Response type
        camel_function_name
    );

    // Only return implementations for HTTP endpoints
    if signature.attr_type == "http" {
        (request_interface, response_type, function_impl)
    } else {
        debug!("Skipping non-HTTP endpoint");
        (String::new(), String::new(), String::new())
    }
}

// Public entry point for creating TypeScript caller-utils
#[instrument(level = "trace", skip_all)]
pub fn create_typescript_caller_utils(base_dir: &Path, api_dir: &Path) -> Result<()> {
    // Path to the new TypeScript file
    let ui_target_dir = base_dir.join("target").join("ui");
    let caller_utils_path = ui_target_dir.join("caller-utils.ts");

    debug!(
        api_dir = %api_dir.display(),
        call_utils_path = %caller_utils_path.display(),
        "Creating TypeScript caller-utils"
    );

    // Find all WIT files in the api directory
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
        "Found WIT interface files for TypeScript generation"
    );

    // Generate TypeScript content
    let mut ts_content = String::new();

    // Add the header with common utilities (always present)
    ts_content.push_str("// Define a custom error type for API errors\n");
    ts_content.push_str("export class ApiError extends Error {\n");
    ts_content.push_str("  constructor(message: string, public readonly details?: unknown) {\n");
    ts_content.push_str("    super(message);\n");
    ts_content.push_str("    this.name = 'ApiError';\n");
    ts_content.push_str("  }\n");
    ts_content.push_str("}\n\n");

    ts_content.push_str("// Parser for the Result-style responses\n");
    ts_content.push_str("// eslint-disable-next-line @typescript-eslint/no-explicit-any\n");
    ts_content.push_str("export function parseResultResponse<T>(response: any): T {\n");
    ts_content.push_str(
        "  if ('Ok' in response && response.Ok !== undefined && response.Ok !== null) {\n",
    );
    ts_content.push_str("    return response.Ok as T;\n");
    ts_content.push_str("  }\n\n");
    ts_content.push_str("  if ('Err' in response && response.Err !== undefined) {\n");
    ts_content.push_str("    throw new ApiError(`API returned an error`, response.Err);\n");
    ts_content.push_str("  }\n\n");
    ts_content.push_str("  throw new ApiError('Invalid API response format');\n");
    ts_content.push_str("}\n\n");

    ts_content.push_str("/**\n");
    ts_content.push_str(" * Generic API request function\n");
    ts_content.push_str(" * @param endpoint - API endpoint\n");
    ts_content.push_str(" * @param method - HTTP method (GET, POST, PUT, DELETE, etc.)\n");
    ts_content.push_str(" * @param data - Request data\n");
    ts_content.push_str(" * @returns Promise with parsed response data\n");
    ts_content.push_str(" * @throws ApiError if the request fails or response contains an error\n");
    ts_content.push_str(" */\n");
    ts_content.push_str("async function apiRequest<T, R>(endpoint: string, method: string, data: T): Promise<R> {\n");
    ts_content
        .push_str("  const BASE_URL = import.meta.env.BASE_URL || window.location.origin;\n\n");
    ts_content.push_str("  const requestOptions: RequestInit = {\n");
    ts_content.push_str("    method: method,\n");
    ts_content.push_str("    headers: {\n");
    ts_content.push_str("      \"Content-Type\": \"application/json\",\n");
    ts_content.push_str("    },\n");
    ts_content.push_str("  };\n\n");
    ts_content.push_str("  // Only add body for methods that support it\n");
    ts_content.push_str("  if (method !== 'GET' && method !== 'HEAD') {\n");
    ts_content.push_str("    requestOptions.body = JSON.stringify(data);\n");
    ts_content.push_str("  }\n\n");
    ts_content.push_str("  const result = await fetch(`${BASE_URL}/api`, requestOptions);\n\n");
    ts_content.push_str("  if (!result.ok) {\n");
    ts_content
        .push_str("    throw new ApiError(`HTTP request failed with status: ${result.status}`);\n");
    ts_content.push_str("  }\n\n");
    ts_content.push_str("  const jsonResponse = await result.json();\n");
    ts_content.push_str("  return parseResultResponse<R>(jsonResponse);\n");
    ts_content.push_str("}\n\n");

    // Collect all interfaces, types, and functions
    let mut all_interfaces = Vec::new();
    let mut all_types = Vec::new();
    let mut all_functions = Vec::new();
    let mut function_names = Vec::new();

    // Generate content for each WIT file
    for wit_file in &wit_files {
        match parse_wit_file(wit_file) {
            Ok(signatures) => {
                for signature in signatures {
                    let (interface_def, type_def, function_def) =
                        generate_typescript_function(&signature);

                    if !interface_def.is_empty() {
                        all_interfaces.push(interface_def);
                        all_types.push(type_def);
                        all_functions.push(function_def);
                        function_names.push(to_camel_case(&signature.function_name));
                    }
                }
            }
            Err(e) => {
                warn!(file = %wit_file.display(), error = %e, "Error parsing WIT file, skipping");
            }
        }
    }

    // If no HTTP functions were found, don't generate the file
    if all_functions.is_empty() {
        debug!("No HTTP functions found in WIT files, skipping TypeScript generation");
        return Ok(());
    }

    // Create directories only after we know we have HTTP functions
    fs::create_dir_all(&ui_target_dir)?;
    debug!("Created UI target directory structure");

    // Add all collected definitions
    if !all_interfaces.is_empty() {
        ts_content.push_str("\n// API Interface Definitions\n\n");
        ts_content.push_str(&all_interfaces.join("\n\n"));
        ts_content.push_str("\n\n");
        ts_content.push_str(&all_types.join("\n\n"));
        ts_content.push_str("\n\n");
    }

    if !all_functions.is_empty() {
        ts_content.push_str("// API Function Implementations\n\n");
        ts_content.push_str(&all_functions.join("\n\n"));
        ts_content.push_str("\n\n");
    }

    // No need for explicit exports since functions are already exported inline

    // Write the TypeScript file
    debug!(
        "Writing generated TypeScript code to {}",
        caller_utils_path.display()
    );
    fs::write(&caller_utils_path, ts_content).with_context(|| {
        format!(
            "Failed to write caller-utils.ts: {}",
            caller_utils_path.display()
        )
    })?;

    info!(
        "Successfully created TypeScript caller-utils at {}",
        caller_utils_path.display()
    );
    Ok(())
}
