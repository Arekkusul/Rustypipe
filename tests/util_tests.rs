use rustypipe::util::{interpolate_command, evaluate_condition};
use std::collections::HashMap;
use std::env;
use std::fs;

#[test]
fn test_simple_output_interpolation() {
    let mut outputs = HashMap::new();
    outputs.insert("task1".to_string(), "hello world".to_string());

    let vars = HashMap::new();

    let result = interpolate_command("echo {{task1.output}}", &outputs, &vars);
    assert_eq!(result, "echo hello world");
}

#[test]
fn test_json_path_interpolation() {
    let mut outputs = HashMap::new();
    outputs.insert("config".to_string(), r#"{"database": {"host": "localhost", "port": 5432}, "debug": true}"#.to_string());

    let vars = HashMap::new();

    // Test nested field access
    let result = interpolate_command("connect to {{config.output.database.host}}", &outputs, &vars);
    assert_eq!(result, "connect to localhost");

    // Test number field
    let result = interpolate_command("port {{config.output.database.port}}", &outputs, &vars);
    assert_eq!(result, "port 5432");

    // Test boolean field
    let result = interpolate_command("debug={{config.output.debug}}", &outputs, &vars);
    assert_eq!(result, "debug=true");
}

#[test]
fn test_json_path_with_array() {
    let mut outputs = HashMap::new();
    outputs.insert("data".to_string(), r#"{"items": [{"name": "item1"}, {"name": "item2"}], "count": 2}"#.to_string());

    let vars = HashMap::new();

    // Test number field
    let result = interpolate_command("total: {{data.output.count}}", &outputs, &vars);
    assert_eq!(result, "total: 2");
}

#[test]
fn test_multiple_interpolations() {
    let mut outputs = HashMap::new();
    outputs.insert("config".to_string(), r#"{"host": "example.com", "port": 8080}"#.to_string());
    outputs.insert("creds".to_string(), r#"{"user": "admin", "pass": "secret"}"#.to_string());

    let vars = HashMap::new();

    let result = interpolate_command(
        "curl http://{{config.output.host}}:{{config.output.port}} -u {{creds.output.user}}:{{creds.output.pass}}",
        &outputs,
        &vars
    );
    assert_eq!(result, "curl http://example.com:8080 -u admin:secret");
}

#[test]
fn test_fallback_for_plain_text() {
    let mut outputs = HashMap::new();
    outputs.insert("task1".to_string(), "plain text output".to_string());

    let vars = HashMap::new();

    // Should return full output when no JSON path
    let result = interpolate_command("result: {{task1.output}}", &outputs, &vars);
    assert_eq!(result, "result: plain text output");
}

#[test]
fn test_missing_json_path() {
    let mut outputs = HashMap::new();
    outputs.insert("config".to_string(), r#"{"host": "localhost"}"#.to_string());

    let vars = HashMap::new();

    // Non-existent path should return empty string
    let result = interpolate_command("port: {{config.output.port}}", &outputs, &vars);
    assert_eq!(result, "port: ");
}

#[test]
fn test_vars_interpolation() {
    let outputs = HashMap::new();

    let mut vars = HashMap::new();
    vars.insert("API_KEY".to_string(), "secret123".to_string());
    vars.insert("ENV".to_string(), "production".to_string());

    let result = interpolate_command("curl -H 'Key: {{vars.API_KEY}}' api.{{vars.ENV}}.com", &outputs, &vars);
    assert_eq!(result, "curl -H 'Key: secret123' api.production.com");
}

#[test]
fn test_mixed_interpolation() {
    let mut outputs = HashMap::new();
    outputs.insert("config".to_string(), r#"{"endpoint": "/api/v1"}"#.to_string());

    let mut vars = HashMap::new();
    vars.insert("HOST".to_string(), "example.com".to_string());

    let result = interpolate_command("curl {{vars.HOST}}{{config.output.endpoint}}", &outputs, &vars);
    assert_eq!(result, "curl example.com/api/v1");
}

#[test]
fn test_env_variable_interpolation() {
    env::set_var("TEST_API_KEY", "env_secret_123");
    env::set_var("TEST_PORT", "9000");

    let outputs = HashMap::new();
    let vars = HashMap::new();

    let result = interpolate_command("curl -H 'Key: ${ENV:TEST_API_KEY}' localhost:${ENV:TEST_PORT}", &outputs, &vars);
    assert_eq!(result, "curl -H 'Key: env_secret_123' localhost:9000");

    env::remove_var("TEST_API_KEY");
    env::remove_var("TEST_PORT");
}

#[test]
fn test_env_variable_missing() {
    let outputs = HashMap::new();
    let vars = HashMap::new();

    // Non-existent env var should return empty string
    let result = interpolate_command("value: ${ENV:NONEXISTENT_VAR}", &outputs, &vars);
    assert_eq!(result, "value: ");
}

#[test]
fn test_file_read_interpolation() {
    // Create a temporary test file
    let test_file = "/tmp/rustypipe_test_secret.txt";
    fs::write(test_file, "file_secret_content").unwrap();

    let outputs = HashMap::new();
    let vars = HashMap::new();

    let result = interpolate_command("secret: ${FILE:/tmp/rustypipe_test_secret.txt}", &outputs, &vars);
    assert_eq!(result, "secret: file_secret_content");

    // Cleanup
    fs::remove_file(test_file).ok();
}

#[test]
fn test_file_read_missing() {
    let outputs = HashMap::new();
    let vars = HashMap::new();

    // Non-existent file should return empty string
    let result = interpolate_command("secret: ${FILE:/nonexistent/file.txt}", &outputs, &vars);
    assert_eq!(result, "secret: ");
}

#[test]
fn test_combined_interpolation_all_types() {
    env::set_var("TEST_ENV", "production");

    let test_file = "/tmp/rustypipe_test_token.txt";
    fs::write(test_file, "token123").unwrap();

    let mut outputs = HashMap::new();
    outputs.insert("config".to_string(), r#"{"port": 8080}"#.to_string());

    let mut vars = HashMap::new();
    vars.insert("HOST".to_string(), "example.com".to_string());

    let result = interpolate_command(
        "curl http://{{vars.HOST}}:{{config.output.port}} -H 'Env: ${ENV:TEST_ENV}' -H 'Token: ${FILE:/tmp/rustypipe_test_token.txt}'",
        &outputs,
        &vars
    );
    assert_eq!(result, "curl http://example.com:8080 -H 'Env: production' -H 'Token: token123'");

    env::remove_var("TEST_ENV");
    fs::remove_file(test_file).ok();
}

#[test]
fn test_condition_equality() {
    let outputs = HashMap::new();

    let mut vars = HashMap::new();
    vars.insert("ENV".to_string(), "production".to_string());

    // Test string equality
    assert!(evaluate_condition("{{vars.ENV}} == 'production'", &outputs, &vars));
    assert!(!evaluate_condition("{{vars.ENV}} == 'development'", &outputs, &vars));

    // Test inequality
    assert!(evaluate_condition("{{vars.ENV}} != 'development'", &outputs, &vars));
    assert!(!evaluate_condition("{{vars.ENV}} != 'production'", &outputs, &vars));
}

#[test]
fn test_condition_numeric_comparison() {
    let mut outputs = HashMap::new();
    outputs.insert("task1".to_string(), "42".to_string());

    let vars = HashMap::new();

    // Test numeric comparisons
    assert!(evaluate_condition("{{task1.output}} > 40", &outputs, &vars));
    assert!(!evaluate_condition("{{task1.output}} > 50", &outputs, &vars));
    assert!(evaluate_condition("{{task1.output}} < 50", &outputs, &vars));
    assert!(!evaluate_condition("{{task1.output}} < 40", &outputs, &vars));
    assert!(evaluate_condition("{{task1.output}} >= 42", &outputs, &vars));
    assert!(evaluate_condition("{{task1.output}} <= 42", &outputs, &vars));
}

#[test]
fn test_condition_boolean_values() {
    let outputs = HashMap::new();
    let vars = HashMap::new();

    // Test literal boolean
    assert!(evaluate_condition("true", &outputs, &vars));
    assert!(!evaluate_condition("false", &outputs, &vars));
    assert!(!evaluate_condition("0", &outputs, &vars));
    assert!(!evaluate_condition("", &outputs, &vars));
    assert!(evaluate_condition("any_text", &outputs, &vars));
}

#[test]
fn test_condition_with_json_path() {
    let mut outputs = HashMap::new();
    outputs.insert("config".to_string(), r#"{"env": "production", "port": 8080}"#.to_string());

    let vars = HashMap::new();

    // Test JSON path in conditions
    assert!(evaluate_condition("{{config.output.env}} == 'production'", &outputs, &vars));
    assert!(!evaluate_condition("{{config.output.env}} == 'development'", &outputs, &vars));
    assert!(evaluate_condition("{{config.output.port}} == 8080", &outputs, &vars));
    assert!(evaluate_condition("{{config.output.port}} > 8000", &outputs, &vars));
}

#[test]
fn test_condition_with_vars_and_outputs() {
    let mut outputs = HashMap::new();
    outputs.insert("build".to_string(), "success".to_string());

    let mut vars = HashMap::new();
    vars.insert("DEPLOY".to_string(), "true".to_string());

    // Combined conditions
    assert!(evaluate_condition("{{build.output}} == 'success'", &outputs, &vars));
    assert!(evaluate_condition("{{vars.DEPLOY}} == 'true'", &outputs, &vars));
}
