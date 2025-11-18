use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use color_eyre::{eyre::WrapErr, Result};
use tracing::{debug, info, instrument, warn};

use walkdir::WalkDir;

// Strip % prefix from WIT identifiers (used to escape keywords)
fn strip_wit_escape(s: &str) -> &str {
    s.strip_prefix('%').unwrap_or(s)
}

// Convert kebab-case to snake_case
pub fn to_snake_case(s: &str) -> String {
    // Strip % prefix if present
    let s = strip_wit_escape(s);

    s.chars().map(|c| if c == '-' { '_' } else { c }).collect()
}

// Convert kebab-case to PascalCase
pub fn to_pascal_case(s: &str) -> String {
    // Strip % prefix if present
    let s = strip_wit_escape(s);

    let parts: Vec<&str> = s.split('-').collect();
    let mut result = String::new();

    for part in parts {
        if part.is_empty() {
            continue;
        }

        // Single letter parts should be uppercased entirely (part of an acronym)
        if part.len() == 1 {
            result.push(part.chars().next().unwrap().to_uppercase().next().unwrap());
        } else {
            // Multi-letter parts: capitalize first letter, keep the rest as-is
            let mut chars = part.chars();
            if let Some(first_char) = chars.next() {
                result.push(first_char.to_uppercase().next().unwrap());
                result.extend(chars);
            }
        }
    }

    result
}

// Extract hyperapp name from WIT filename
fn extract_hyperapp_name(wit_file_path: &Path) -> Option<String> {
    wit_file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|filename| {
            // Remove -sys-v0 suffix if present
            let name = if filename.ends_with("-sys-v0") {
                &filename[..filename.len() - 7]
            } else {
                filename
            };

            // Skip types- prefix files
            if name.starts_with("types-") {
                return extract_hyperapp_name_from_types(name);
            }

            // Convert to PascalCase for namespace name
            to_pascal_case(name)
        })
}

// Extract hyperapp name from types- prefixed files
fn extract_hyperapp_name_from_types(filename: &str) -> String {
    // types-spider-sys-v0 -> Spider
    // types-ttstt-sys-v0 -> Ttstt
    let name = filename.strip_prefix("types-").unwrap_or(filename);
    let name = if name.ends_with("-sys-v0") {
        &name[..name.len() - 7]
    } else {
        name
    };
    to_pascal_case(name)
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
            // Find the comma that separates Ok and Err types, handling nested generics
            let mut depth = 0;
            let mut comma_pos = None;

            for (i, ch) in inner_part.chars().enumerate() {
                match ch {
                    '<' => depth += 1,
                    '>' => depth -= 1,
                    ',' if depth == 0 => {
                        comma_pos = Some(i);
                        break;
                    }
                    _ => {}
                }
            }

            if let Some(pos) = comma_pos {
                let ok_type = inner_part[..pos].trim();
                let err_type = inner_part[pos + 1..].trim();
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
            // Parse tuple elements correctly, handling nested generics
            let mut elements = Vec::new();
            let mut current = String::new();
            let mut depth = 0;

            for ch in inner_types.chars() {
                match ch {
                    '<' => {
                        depth += 1;
                        current.push(ch);
                    }
                    '>' => {
                        depth -= 1;
                        current.push(ch);
                    }
                    ',' if depth == 0 => {
                        // Only split on commas at the top level
                        elements.push(current.trim().to_string());
                        current.clear();
                    }
                    _ => {
                        current.push(ch);
                    }
                }
            }
            // Don't forget the last element
            if !current.trim().is_empty() {
                elements.push(current.trim().to_string());
            }

            let ts_types: Vec<String> =
                elements.iter().map(|t| wit_type_to_typescript(t)).collect();
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
        // Find the comma that separates Ok and Err types, handling nested generics
        let mut depth = 0;
        let mut comma_pos = None;

        for (i, ch) in inner_part.chars().enumerate() {
            match ch {
                '<' => depth += 1,
                '>' => depth -= 1,
                ',' if depth == 0 => {
                    comma_pos = Some(i);
                    break;
                }
                _ => {}
            }
        }

        if let Some(pos) = comma_pos {
            let ok_type = inner_part[..pos].trim();
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
    http_method: Option<String>,
    http_path: Option<String>,
}

// Structure to represent a WIT record
#[derive(Debug)]
struct WitRecord {
    name: String,
    fields: Vec<SignatureField>,
}

// Structure to represent a WIT variant case with optional data
#[derive(Debug)]
struct WitVariantCase {
    name: String,
    data_type: Option<String>,
}

// Structure to represent a WIT variant
#[derive(Debug)]
struct WitVariant {
    name: String,
    cases: Vec<WitVariantCase>,
}

// Structure to represent a WIT enum (variant without data)
#[derive(Debug)]
struct WitEnum {
    name: String,
    cases: Vec<String>,
}

// Structure to hold all parsed WIT types
struct WitTypes {
    signatures: Vec<SignatureStruct>,
    records: Vec<WitRecord>,
    variants: Vec<WitVariant>,
    enums: Vec<WitEnum>,
    aliases: Vec<(String, String)>,
}

// Structure to hold types grouped by hyperapp
struct HyperappTypes {
    _name: String,
    signatures: Vec<SignatureStruct>,
    records: Vec<WitRecord>,
    variants: Vec<WitVariant>,
    enums: Vec<WitEnum>,
    aliases: Vec<(String, String)>,
}

// Parse WIT file to extract function signatures, records, and variants
#[instrument(level = "trace", skip_all)]
fn parse_wit_file(file_path: &Path) -> Result<WitTypes> {
    debug!(file = %file_path.display(), "Parsing WIT file");

    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read WIT file: {}", file_path.display()))?;

    let mut signatures = Vec::new();
    let mut records = Vec::new();
    let mut variants = Vec::new();
    let mut enums = Vec::new();
    let mut aliases = Vec::new();

    // Simple parser for WIT files to extract record definitions
    let lines: Vec<_> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Look for type aliases
        if line.starts_with("type ") {
            // Expect: type name = rhs
            let rest = line
                .trim_start_matches("type ")
                .trim_end_matches(';')
                .trim();
            if let Some(eq_pos) = rest.find('=') {
                let name = strip_wit_escape(rest[..eq_pos].trim()).to_string();
                let rhs = rest[eq_pos + 1..].trim().to_string();
                debug!(alias = %name, rhs = %rhs, "Found alias");
                aliases.push((name, rhs));
            }
        }
        // Look for record definitions
        else if line.starts_with("record ") {
            let record_name = line
                .trim_start_matches("record ")
                .trim_end_matches(" {")
                .trim();

            // Strip % prefix if present
            let record_name = strip_wit_escape(record_name);

            if record_name.contains("-signature-") {
                // This is a signature record
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

                let mut http_method = None;
                let mut http_path = None;

                // scan backward/upward to get method/path from a // HTTP: comment
                if attr_type == "http" {
                    let mut j = i;
                    while j > 0 {
                        let prev_line = lines[j - 1].trim();
                        if prev_line.is_empty() {
                            j -= 1;
                            continue;
                        }
                        if prev_line.starts_with("// HTTP:") {
                            let rest = prev_line.trim_start_matches("// HTTP:").trim();
                            let tokens: Vec<&str> = rest.split_whitespace().collect();
                            if let Some(method_token) = tokens.first() {
                                http_method = Some(method_token.to_uppercase());
                            }
                            if let Some(path_token) = tokens.get(1) {
                                http_path = Some(path_token.to_string());
                            }
                            break;
                        } else if prev_line.starts_with("//") {
                            j -= 1;
                            continue;
                        } else {
                            break;
                        }
                    }
                }

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
                        let field_name = strip_wit_escape(field_parts[0].trim()).to_string();
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
                    http_method,
                    http_path,
                });
            } else {
                // This is a regular record
                debug!(name = %record_name, "Found record");

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
                        let field_name = strip_wit_escape(field_parts[0].trim()).to_string();
                        let field_type = field_parts[1].trim().trim_end_matches(',').to_string();

                        debug!(name = %field_name, wit_type = %field_type, "Found field");
                        fields.push(SignatureField {
                            name: field_name,
                            wit_type: field_type,
                        });
                    }

                    i += 1;
                }

                records.push(WitRecord {
                    name: record_name.to_string(),
                    fields,
                });
            }
        }
        // Look for variant definitions
        else if line.starts_with("variant ") {
            let variant_name = line
                .trim_start_matches("variant ")
                .trim_end_matches(" {")
                .trim();

            // Strip % prefix if present
            let variant_name = strip_wit_escape(variant_name);
            debug!(name = %variant_name, "Found variant");

            // Parse cases
            let mut cases = Vec::new();
            i += 1;

            while i < lines.len() && !lines[i].trim().starts_with("}") {
                let case_line = lines[i].trim();

                // Skip comments and empty lines
                if case_line.starts_with("//") || case_line.is_empty() {
                    i += 1;
                    continue;
                }

                // Parse case with optional associated data
                let case_raw = case_line.trim_end_matches(',');

                let (case_name, data_type) = if let Some(paren_pos) = case_raw.find('(') {
                    let name = strip_wit_escape(&case_raw[..paren_pos]).to_string();
                    // Extract the type between parentheses
                    let type_end = case_raw.rfind(')').unwrap_or(case_raw.len());
                    let type_str = &case_raw[paren_pos + 1..type_end];
                    (name, Some(type_str.to_string()))
                } else {
                    (strip_wit_escape(case_raw).to_string(), None)
                };

                debug!(case = %case_name, data_type = ?data_type, "Found variant case");
                cases.push(WitVariantCase {
                    name: case_name,
                    data_type,
                });

                i += 1;
            }

            variants.push(WitVariant {
                name: variant_name.to_string(),
                cases,
            });
        }
        // Look for enum definitions
        else if line.starts_with("enum ") {
            let enum_name = line
                .trim_start_matches("enum ")
                .trim_end_matches(" {")
                .trim();

            // Strip % prefix if present
            let enum_name = strip_wit_escape(enum_name);
            debug!(name = %enum_name, "Found enum");

            // Parse enum cases
            let mut cases = Vec::new();
            i += 1;

            while i < lines.len() && !lines[i].trim().starts_with("}") {
                let case_line = lines[i].trim();

                // Skip comments and empty lines
                if case_line.starts_with("//") || case_line.is_empty() {
                    i += 1;
                    continue;
                }

                // Parse enum case (simple name without data)
                let case_name = strip_wit_escape(case_line.trim_end_matches(',')).to_string();
                debug!(case = %case_name, "Found enum case");
                cases.push(case_name);

                i += 1;
            }

            enums.push(WitEnum {
                name: enum_name.to_string(),
                cases,
            });
        }

        i += 1;
    }

    debug!(
        file = %file_path.display(),
        signatures = signatures.len(),
        records = records.len(),
        variants = variants.len(),
        enums = enums.len(),
        "Finished parsing WIT file"
    );
    Ok(WitTypes {
        signatures,
        records,
        variants,
        enums,
        aliases,
    })
}

// Generate TypeScript interface from a WIT record
fn generate_typescript_interface(record: &WitRecord) -> String {
    let interface_name = to_pascal_case(&record.name);
    let mut fields = Vec::new();

    for field in &record.fields {
        let field_name = to_snake_case(&field.name);
        let ts_type = wit_type_to_typescript(&field.wit_type);
        fields.push(format!("  {}: {};", field_name, ts_type));
    }

    format!(
        "export interface {} {{\n{}\n}}",
        interface_name,
        fields.join("\n")
    )
}

// Generate TypeScript enum from a WIT enum
fn generate_typescript_enum(enum_def: &WitEnum) -> String {
    let type_name = to_pascal_case(&enum_def.name);

    // Generate as TypeScript enum with string values
    let mut enum_str = format!("export enum {} {{\n", type_name);

    for case in &enum_def.cases {
        let case_pascal = to_pascal_case(case);
        // Use the PascalCase value as the string value to match the original Rust enum
        enum_str.push_str(&format!("  {} = \"{}\",\n", case_pascal, case_pascal));
    }

    enum_str.push_str("}");
    enum_str
}

// Generate TypeScript type from a WIT variant
fn generate_typescript_variant(variant: &WitVariant) -> String {
    let type_name = to_pascal_case(&variant.name);

    // Check if this is a simple enum (no associated data) or a tagged union
    let has_data = variant.cases.iter().any(|case| case.data_type.is_some());

    if !has_data {
        // Simple enum - generate as string union
        let cases: Vec<String> = variant
            .cases
            .iter()
            .map(|case| format!("\"{}\"", to_pascal_case(&case.name)))
            .collect();
        format!("export type {} = {};", type_name, cases.join(" | "))
    } else {
        // Tagged union - generate as discriminated union
        let cases: Vec<String> = variant
            .cases
            .iter()
            .map(|case| {
                let case_name = to_pascal_case(&case.name);
                if let Some(ref data_type) = case.data_type {
                    // Handle record types specially
                    if data_type.trim().starts_with("record {") {
                        // Parse record fields from the data type
                        let record_content = data_type.trim_start_matches("record").trim();
                        let fields = parse_inline_record_fields(record_content);
                        format!("{{ {}: {} }}", case_name, fields)
                    } else {
                        // Simple type
                        let ts_type = wit_type_to_typescript(data_type);
                        format!("{{ {}: {} }}", case_name, ts_type)
                    }
                } else {
                    // Case without data - still use object format for consistency
                    format!("{{ {}: null }}", case_name)
                }
            })
            .collect();

        format!("export type {} = {};", type_name, cases.join(" | "))
    }
}

// Helper to parse inline record fields
fn parse_inline_record_fields(record_str: &str) -> String {
    // Remove the curly braces
    let content = record_str
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();

    // Parse each field
    let fields: Vec<String> = content
        .split(',')
        .filter_map(|field| {
            let field = field.trim();
            if field.is_empty() {
                return None;
            }

            // Split field name and type
            if let Some(colon_pos) = field.find(':') {
                let field_name = field[..colon_pos].trim();
                let field_type = field[colon_pos + 1..].trim();
                let field_name = strip_wit_escape(field_name);
                let ts_name = to_snake_case(field_name);
                let ts_type = wit_type_to_typescript(field_type);
                Some(format!("{}: {}", ts_name, ts_type))
            } else {
                None
            }
        })
        .collect();

    format!("{{ {} }}", fields.join(", "))
}

// Generate TypeScript interface and function from a signature struct
fn generate_typescript_function(
    signature: &SignatureStruct,
    _use_namespace: bool,
) -> (String, String, String) {
    // Convert function name from kebab-case to camelCase
    let camel_function_name = to_snake_case(&signature.function_name);
    let pascal_function_name = to_pascal_case(&signature.function_name);

    debug!(name = %camel_function_name, "Generating TypeScript function");

    // Extract parameters and return type
    let mut params = Vec::new();
    let mut param_names = Vec::new();
    let mut param_types = Vec::new();
    let mut full_return_type = "void".to_string();
    let mut unwrapped_return_type = "void".to_string();

    let http_method = signature
        .http_method
        .clone()
        .unwrap_or_else(|| "POST".to_string());
    let http_path = signature
        .http_path
        .clone()
        .unwrap_or_else(|| "/api".to_string());

    let actual_param_type: String;

    for field in &signature.fields {
        let field_name_camel = to_snake_case(&field.name);
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

    // Determine the actual parameter type for the function
    if param_names.is_empty() {
        actual_param_type = "null".to_string();
    } else if param_names.len() == 1 {
        actual_param_type = param_types[0].clone();
    } else {
        actual_param_type = format!("[{}]", param_types.join(", "));
    }

    // Generate request interface with a different name to avoid conflicts
    let request_interface_name = format!("{}RequestWrapper", pascal_function_name);
    let request_interface = format!(
        "export interface {} {{\n  {}: {}\n}}",
        request_interface_name, pascal_function_name, actual_param_type
    );

    // Generate response type alias (using the full Result type)
    let response_type = format!(
        "export type {}Response = {};",
        pascal_function_name, full_return_type
    );

    // Generate function implementation
    let function_params = params.join(", ");

    let data_construction = if param_names.is_empty() {
        format!(
            "  const data: {} = {{\n    {}: null,\n  }};",
            request_interface_name, pascal_function_name
        )
    } else if param_names.len() == 1 {
        format!(
            "  const data: {} = {{\n    {}: {},\n  }};",
            request_interface_name, pascal_function_name, param_names[0]
        )
    } else {
        format!(
            "  const data: {} = {{\n    {}: [{}],\n  }};",
            request_interface_name,
            pascal_function_name,
            param_names.join(", ")
        )
    };

    // Function returns the unwrapped type since parseResponse extracts it
    let function_impl = format!(
        "/**\n * {}\n{} * @returns Promise with result\n * @throws ApiError if the request fails\n */\nexport async function {}({}): Promise<{}> {{\n{}\n\n  return await apiRequest<{}, {}>('{}', '{}', data);\n}}",
        camel_function_name,
        params.iter().map(|p| format!(" * @param {}", p)).collect::<Vec<_>>().join("\n"),
        camel_function_name,
        function_params,
        unwrapped_return_type,  // Use unwrapped type as the function return
        data_construction,
        request_interface_name,
        unwrapped_return_type,  // Pass unwrapped type to apiRequest, not Response type
        http_path,
        http_method
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

    // Find all WIT files in the api directory and group by hyperapp
    let mut hyperapp_files: HashMap<String, Vec<PathBuf>> = HashMap::new();

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
                    // Extract hyperapp name from filename
                    if let Some(hyperapp_name) = extract_hyperapp_name(path) {
                        debug!(file = %path.display(), hyperapp = %hyperapp_name, "Adding WIT file for parsing");
                        hyperapp_files
                            .entry(hyperapp_name)
                            .or_insert_with(Vec::new)
                            .push(path.to_path_buf());
                    }
                } else {
                    debug!(file = %path.display(), "Skipping world definition WIT file");
                }
            }
        }
    }

    debug!(
        hyperapps = hyperapp_files.len(),
        "Found hyperapps for TypeScript generation"
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
    ts_content.push_str("export function parseResponse<T>(response: any): T {\n");
    ts_content.push_str("  try {\n");
    ts_content.push_str(
        "    if ('Ok' in response && response.Ok !== undefined && response.Ok !== null) {\n",
    );
    ts_content.push_str("      return response.Ok as T;\n");
    ts_content.push_str("    }\n\n");
    ts_content.push_str("    if ('Err' in response && response.Err !== undefined) {\n");
    ts_content.push_str("      throw new ApiError(`API returned an error`, response.Err);\n");
    ts_content.push_str("    }\n");
    ts_content.push_str("  } catch (e) {\n");
    ts_content.push_str("    return response as T;\n");
    ts_content.push_str("  }\n");
    ts_content.push_str("  return response as T;\n");
    ts_content.push_str("}\n\n");

    ts_content.push_str("/**\n");
    ts_content.push_str(" * Generic API request function\n");
    ts_content.push_str(" * @param path - API endpoint path\n");
    ts_content.push_str(" * @param method - HTTP method (GET, POST, PUT, DELETE, etc.)\n");
    ts_content.push_str(" * @param data - Request data\n");
    ts_content.push_str(" * @returns Promise with parsed response data\n");
    ts_content.push_str(" * @throws ApiError if the request fails or response contains an error\n");
    ts_content.push_str(" */\n");
    ts_content.push_str(
        "async function apiRequest<T, R>(path: string, method: string, data: T): Promise<R> {\n",
    );
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
    ts_content.push_str(
        "  const url = path.startsWith('/') ? `${BASE_URL}${path}` : `${BASE_URL}/${path}`;\n",
    );
    ts_content.push_str("  const result = await fetch(url, requestOptions);\n\n");
    ts_content.push_str("  if (!result.ok) {\n");
    ts_content
        .push_str("    throw new ApiError(`HTTP request failed with status: ${result.status}`);\n");
    ts_content.push_str("  }\n\n");
    ts_content.push_str("  const jsonResponse = await result.json();\n");
    ts_content.push_str("  return parseResponse<R>(jsonResponse);\n");
    ts_content.push_str("}\n\n");

    // Collect types grouped by hyperapp
    let mut hyperapp_types_map: HashMap<String, HyperappTypes> = HashMap::new();
    let mut has_any_functions = false;

    // Process WIT files grouped by hyperapp
    for (hyperapp_name, wit_files) in &hyperapp_files {
        let mut hyperapp_data = HyperappTypes {
            _name: hyperapp_name.clone(),
            signatures: Vec::new(),
            records: Vec::new(),
            variants: Vec::new(),
            enums: Vec::new(),
            aliases: Vec::new(),
        };

        // Parse each WIT file for this hyperapp
        for wit_file in wit_files {
            match parse_wit_file(wit_file) {
                Ok(wit_types) => {
                    // Check for conflicting type names
                    for record in &wit_types.records {
                        let type_name = to_pascal_case(&record.name);
                        if type_name.ends_with("Request") || type_name.ends_with("Response") {
                            return Err(color_eyre::eyre::eyre!(
                                "Type '{}' in {} has a reserved suffix (Request/Response). \
                                These suffixes are reserved for generated wrapper types. \
                                Please rename the type in the WIT file.",
                                record.name,
                                wit_file.display()
                            ));
                        }
                        if type_name.ends_with("RequestWrapper")
                            || type_name.ends_with("ResponseWrapper")
                        {
                            return Err(color_eyre::eyre::eyre!(
                                "Type '{}' in {} has a reserved suffix (RequestWrapper/ResponseWrapper). \
                                These suffixes are reserved for generated types. \
                                Please rename the type in the WIT file.",
                                record.name, wit_file.display()
                            ));
                        }
                    }

                    for variant in &wit_types.variants {
                        let type_name = to_pascal_case(&variant.name);
                        if type_name.ends_with("Request") || type_name.ends_with("Response") {
                            return Err(color_eyre::eyre::eyre!(
                                "Type '{}' in {} has a reserved suffix (Request/Response). \
                                These suffixes are reserved for generated wrapper types. \
                                Please rename the type in the WIT file.",
                                variant.name,
                                wit_file.display()
                            ));
                        }
                        if type_name.ends_with("RequestWrapper")
                            || type_name.ends_with("ResponseWrapper")
                        {
                            return Err(color_eyre::eyre::eyre!(
                                "Type '{}' in {} has a reserved suffix (RequestWrapper/ResponseWrapper). \
                                These suffixes are reserved for generated types. \
                                Please rename the type in the WIT file.",
                                variant.name, wit_file.display()
                            ));
                        }
                    }

                    for enum_def in &wit_types.enums {
                        let type_name = to_pascal_case(&enum_def.name);
                        if type_name.ends_with("Request") || type_name.ends_with("Response") {
                            return Err(color_eyre::eyre::eyre!(
                                "Type '{}' in {} has a reserved suffix (Request/Response). \
                                These suffixes are reserved for generated wrapper types. \
                                Please rename the type in the WIT file.",
                                enum_def.name,
                                wit_file.display()
                            ));
                        }
                        if type_name.ends_with("RequestWrapper")
                            || type_name.ends_with("ResponseWrapper")
                        {
                            return Err(color_eyre::eyre::eyre!(
                                "Type '{}' in {} has a reserved suffix (RequestWrapper/ResponseWrapper). \
                                These suffixes are reserved for generated types. \
                                Please rename the type in the WIT file.",
                                enum_def.name, wit_file.display()
                            ));
                        }
                    }

                    // Collect all types for this hyperapp
                    hyperapp_data.records.extend(wit_types.records);
                    hyperapp_data.aliases.extend(wit_types.aliases);
                    hyperapp_data.variants.extend(wit_types.variants);
                    hyperapp_data.enums.extend(wit_types.enums);

                    // Only collect HTTP signatures
                    for sig in wit_types.signatures {
                        if sig.attr_type == "http" {
                            hyperapp_data.signatures.push(sig);
                            has_any_functions = true;
                        }
                    }
                }
                Err(e) => {
                    warn!(file = %wit_file.display(), error = %e, "Error parsing WIT file, skipping");
                }
            }
        }

        if !hyperapp_data.signatures.is_empty()
            || !hyperapp_data.records.is_empty()
            || !hyperapp_data.variants.is_empty()
            || !hyperapp_data.enums.is_empty()
            || !hyperapp_data.aliases.is_empty()
        {
            hyperapp_types_map.insert(hyperapp_name.clone(), hyperapp_data);
        }
    }

    // If no HTTP functions were found, don't generate the file
    if !has_any_functions {
        debug!("No HTTP functions found in WIT files, skipping TypeScript generation");
        return Ok(());
    }

    // Create directories only after we know we have HTTP functions
    fs::create_dir_all(&ui_target_dir)?;
    debug!("Created UI target directory structure");

    // Generate TypeScript namespaces for each hyperapp
    for (hyperapp_name, hyperapp_data) in &hyperapp_types_map {
        ts_content.push_str(&format!(
            "\n// ============= {} Hyperapp =============\n",
            hyperapp_name
        ));
        ts_content.push_str(&format!("export namespace {} {{\n", hyperapp_name));

        // Add custom types (aliases, records, variants, and enums) for this hyperapp
        if !hyperapp_data.aliases.is_empty()
            || !hyperapp_data.records.is_empty()
            || !hyperapp_data.variants.is_empty()
            || !hyperapp_data.enums.is_empty()
        {
            ts_content.push_str("\n  // Custom Types\n");

            // Generate type aliases first so downstream types can reference them
            for (alias_name, rhs) in &hyperapp_data.aliases {
                let ts_alias = to_pascal_case(alias_name);
                // Special-case: map WIT alias `value` to TS `unknown` for ergonomic JSON usage
                let rhs_ts = if alias_name == "value" {
                    "unknown".to_string()
                } else {
                    wit_type_to_typescript(rhs)
                };
                ts_content.push_str(&format!("  export type {} = {}\n", ts_alias, rhs_ts));
            }
            if !hyperapp_data.aliases.is_empty() {
                ts_content.push_str("\n");
            }

            // Generate enums first
            for enum_def in &hyperapp_data.enums {
                let enum_ts = generate_typescript_enum(enum_def);
                // Indent the enum definition for namespace
                let indented = enum_ts
                    .lines()
                    .map(|line| {
                        if line.is_empty() {
                            String::new()
                        } else {
                            format!("  {}", line)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                ts_content.push_str(&indented);
                ts_content.push_str("\n\n");
            }

            for record in &hyperapp_data.records {
                let interface_def = generate_typescript_interface(record);
                // Indent the interface definition for namespace
                let indented = interface_def
                    .lines()
                    .map(|line| {
                        if line.is_empty() {
                            String::new()
                        } else {
                            format!("  {}", line)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                ts_content.push_str(&indented);
                ts_content.push_str("\n\n");
            }

            for variant in &hyperapp_data.variants {
                let type_def = generate_typescript_variant(variant);
                // Indent the type definition for namespace
                let indented = type_def
                    .lines()
                    .map(|line| {
                        if line.is_empty() {
                            String::new()
                        } else {
                            format!("  {}", line)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                ts_content.push_str(&indented);
                ts_content.push_str("\n\n");
            }
        }

        // Add request/response interfaces and functions for this hyperapp
        if !hyperapp_data.signatures.is_empty() {
            ts_content.push_str("\n  // API Request/Response Types\n");

            for signature in &hyperapp_data.signatures {
                let (interface_def, type_def, _function_def) =
                    generate_typescript_function(signature, true);

                if !interface_def.is_empty() {
                    // Indent interface definition
                    let indented = interface_def
                        .lines()
                        .map(|line| {
                            if line.is_empty() {
                                String::new()
                            } else {
                                format!("  {}", line)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    ts_content.push_str(&indented);
                    ts_content.push_str("\n\n");

                    // Indent type definition
                    let indented = type_def
                        .lines()
                        .map(|line| {
                            if line.is_empty() {
                                String::new()
                            } else {
                                format!("  {}", line)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    ts_content.push_str(&indented);
                    ts_content.push_str("\n\n");
                }
            }

            ts_content.push_str("\n  // API Functions\n");

            for signature in &hyperapp_data.signatures {
                let (_, _, function_def) = generate_typescript_function(signature, true);

                if !function_def.is_empty() {
                    // Indent function definition
                    let indented = function_def
                        .lines()
                        .map(|line| {
                            if line.is_empty() {
                                String::new()
                            } else {
                                format!("  {}", line)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    ts_content.push_str(&indented);
                    ts_content.push_str("\n\n");
                }
            }
        }

        // Close namespace
        ts_content.push_str("}\n");
    }

    ts_content.push_str("\n");

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_enum_generation() {
        // Create a temporary directory
        let temp_dir = tempdir().unwrap();
        let api_dir = temp_dir.path().join("api");
        fs::create_dir(&api_dir).unwrap();

        // Create a test WIT file with an enum
        let wit_content = r#"
interface test {
    enum test-enum {
        option-one,
        option-two,
        option-three
    }

    record test-data {
        value: test-enum
    }

    // Function signature for: test-func (http)
    // HTTP: POST /api/test-func
    record test-func-signature-http {
        target: string,
        request: test-data,
        returning: result<string, string>
    }
}
"#;

        let wit_file = api_dir.join("test.wit");
        fs::write(&wit_file, wit_content).unwrap();

        // Generate TypeScript
        let result = create_typescript_caller_utils(temp_dir.path(), &api_dir);
        assert!(
            result.is_ok(),
            "Failed to generate TypeScript: {:?}",
            result
        );

        // Read generated TypeScript file
        let ts_file = temp_dir
            .path()
            .join("target")
            .join("ui")
            .join("caller-utils.ts");
        let ts_content = fs::read_to_string(&ts_file).unwrap();

        // Check that the enum was generated
        assert!(
            ts_content.contains("export enum TestEnum"),
            "Enum not found in generated TypeScript"
        );
        assert!(
            ts_content.contains("OptionOne = \"option-one\""),
            "Enum case OptionOne not found"
        );
        assert!(
            ts_content.contains("OptionTwo = \"option-two\""),
            "Enum case OptionTwo not found"
        );
        assert!(
            ts_content.contains("OptionThree = \"option-three\""),
            "Enum case OptionThree not found"
        );

        // Check that the interface using the enum was generated
        assert!(
            ts_content.contains("export interface TestData"),
            "Interface not found"
        );
        assert!(
            ts_content.contains("value: TestEnum"),
            "Enum reference in interface not found"
        );
    }
}
