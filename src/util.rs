use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;
use std::fs;
use chrono::Utc;
use anyhow::Context;
use serde_json::Value;

/// Extract value from JSON using dot notation path (e.g., "field.subfield.value")
fn extract_json_path(json_val: &Value, path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = json_val;

    for part in parts {
        current = current.get(part)?;
    }

    // Convert final value to string
    match current {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some("null".to_string()),
        _ => Some(current.to_string()),
    }
}

/// Enhanced interpolation: replace {{task.output.path}} and {{vars.NAME}}
/// Supports JSON path access for task outputs using dot notation
/// Supports environment variables: ${ENV:VAR_NAME}
/// Supports file reads: ${FILE:/path/to/file}
pub fn interpolate_command(template: &str, outputs: &HashMap<String, String>, vars: &HashMap<String, String>) -> String {
    let mut s = template.to_string();

    // Step 1: Replace environment variables ${ENV:VAR_NAME}
    let env_pattern = Regex::new(r"\$\{ENV:([a-zA-Z0-9_]+)\}").unwrap();
    s = env_pattern.replace_all(&s, |caps: &regex::Captures| {
        let var_name = &caps[1];
        std::env::var(var_name).unwrap_or_default()
    }).to_string();

    // Step 2: Replace file reads ${FILE:/path/to/file}
    let file_pattern = Regex::new(r"\$\{FILE:([^\}]+)\}").unwrap();
    s = file_pattern.replace_all(&s, |caps: &regex::Captures| {
        let file_path = &caps[1];
        match fs::read_to_string(file_path.trim()) {
            Ok(content) => content.trim().to_string(),
            Err(_) => String::new(),
        }
    }).to_string();

    // Step 3: Replace vars
    for (k, v) in vars {
        let p1 = format!("{{{{vars.{}}}}}", k);
        let p2 = format!("{{{{vars.{} }}}}", k);
        s = s.replace(&p1, v);
        s = s.replace(&p2, v);
    }

    // Step 4: Replace outputs with JSON path support
    // Pattern: {{taskname.output.path.to.field}}
    let output_pattern = Regex::new(r"\{\{([a-zA-Z0-9_-]+)\.output(?:\.([a-zA-Z0-9_.-]+))?\s*\}\}").unwrap();

    s = output_pattern.replace_all(&s, |caps: &regex::Captures| {
        let task_name = &caps[1];
        let json_path = caps.get(2).map(|m| m.as_str());

        if let Some(output) = outputs.get(task_name) {
            let output_trimmed = output.trim();

            // If there's a JSON path, try to parse as JSON and extract
            if let Some(path) = json_path {
                if let Ok(json_val) = serde_json::from_str::<Value>(output_trimmed) {
                    if let Some(value) = extract_json_path(&json_val, path) {
                        return value;
                    }
                }
                // If JSON parsing or path extraction fails, return empty string
                return String::new();
            } else {
                // No JSON path, return full output
                return output_trimmed.to_string();
            }
        }

        // Task not found, return empty string
        String::new()
    }).to_string();

    // Step 5: Remove any remaining {{...}} to avoid executing raw templates later
    let re = Regex::new(r"\{\{.*?\}\}").unwrap();
    s = re.replace_all(&s, "").to_string();

    s
}

/// Create a run directory and return it
pub fn create_run_dir(base: &Path) -> anyhow::Result<std::path::PathBuf> {
    let run_id = Uuid::new_v4().to_string();
    let dir = base.join("runs").join(run_id);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn write_artifact(dir: &Path, name: &str, content: &str) -> anyhow::Result<()> {
    let path = dir.join(name);
    fs::write(path, content)?;
    Ok(())
}

pub fn timestamp() -> String {
    // Format: YYYY-MM-DD_HH-MM-SS
    Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string()
}

/// Evaluate a conditional expression
/// Supports: ==, !=, >, <, >=, <=
/// Example: "{{vars.ENV}} == 'production'"
pub fn evaluate_condition(condition: &str, outputs: &HashMap<String, String>, vars: &HashMap<String, String>) -> bool {
    // First interpolate all variables in the condition
    let interpolated = interpolate_command(condition, outputs, vars);

    // Parse and evaluate the condition
    // Support operators: ==, !=, >, <, >=, <=
    let operators = ["==", "!=", ">=", "<=", ">", "<"];

    for op in &operators {
        if let Some(pos) = interpolated.find(op) {
            let left = interpolated[..pos].trim();
            let right = interpolated[pos + op.len()..].trim();

            // Remove quotes from string literals
            let left_val = left.trim_matches('\'').trim_matches('"');
            let right_val = right.trim_matches('\'').trim_matches('"');

            return match *op {
                "==" => left_val == right_val,
                "!=" => left_val != right_val,
                ">" => {
                    // Try numeric comparison first
                    if let (Ok(l), Ok(r)) = (left_val.parse::<f64>(), right_val.parse::<f64>()) {
                        l > r
                    } else {
                        left_val > right_val
                    }
                }
                "<" => {
                    if let (Ok(l), Ok(r)) = (left_val.parse::<f64>(), right_val.parse::<f64>()) {
                        l < r
                    } else {
                        left_val < right_val
                    }
                }
                ">=" => {
                    if let (Ok(l), Ok(r)) = (left_val.parse::<f64>(), right_val.parse::<f64>()) {
                        l >= r
                    } else {
                        left_val >= right_val
                    }
                }
                "<=" => {
                    if let (Ok(l), Ok(r)) = (left_val.parse::<f64>(), right_val.parse::<f64>()) {
                        l <= r
                    } else {
                        left_val <= right_val
                    }
                }
                _ => false,
            };
        }
    }

    // If no operator found, treat as boolean (non-empty string is true)
    !interpolated.trim().is_empty() && interpolated.trim() != "false" && interpolated.trim() != "0"
}
