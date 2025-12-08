use rustypipe::pipeline::parser::{Pipeline, validate_pipeline};
use std::collections::HashMap;

#[test]
fn test_validate_pipeline_with_valid_dag() {
    let mut pipeline = Pipeline {
        name: Some("test".to_string()),
        concurrency: Some(2),
        stop_on_fail: Some(false),
        backends: HashMap::new(),
        secrets: HashMap::new(),
        tasks: vec![
            rustypipe::pipeline::parser::TaskDef {
                name: "task1".to_string(),
                depends_on: vec![],
                run: "echo hello".to_string(),
                retries: None,
                timeout: None,
                backend: None,
                cache_key: None,
                continue_on_fail: None,
                when: None,
                before: None,
                after: None,
                on_failure: None,
            },
            rustypipe::pipeline::parser::TaskDef {
                name: "task2".to_string(),
                depends_on: vec!["task1".to_string()],
                run: "echo world".to_string(),
                retries: None,
                timeout: None,
                backend: None,
                cache_key: None,
                continue_on_fail: None,
                when: None,
                before: None,
                after: None,
                on_failure: None,
            },
        ],
    };

    let result = validate_pipeline(&pipeline);
    assert!(result.is_ok());
}

#[test]
fn test_validate_pipeline_detects_cycle() {
    let pipeline = Pipeline {
        name: Some("test".to_string()),
        concurrency: Some(2),
        stop_on_fail: Some(false),
        backends: HashMap::new(),
        secrets: HashMap::new(),
        tasks: vec![
            rustypipe::pipeline::parser::TaskDef {
                name: "task1".to_string(),
                depends_on: vec!["task2".to_string()],
                run: "echo hello".to_string(),
                retries: None,
                timeout: None,
                backend: None,
                cache_key: None,
                continue_on_fail: None,
                when: None,
                before: None,
                after: None,
                on_failure: None,
            },
            rustypipe::pipeline::parser::TaskDef {
                name: "task2".to_string(),
                depends_on: vec!["task1".to_string()],
                run: "echo world".to_string(),
                retries: None,
                timeout: None,
                backend: None,
                cache_key: None,
                continue_on_fail: None,
                when: None,
                before: None,
                after: None,
                on_failure: None,
            },
        ],
    };

    let result = validate_pipeline(&pipeline);
    assert!(result.is_err());
}

#[test]
fn test_validate_pipeline_detects_missing_dependency() {
    let pipeline = Pipeline {
        name: Some("test".to_string()),
        concurrency: Some(2),
        stop_on_fail: Some(false),
        backends: HashMap::new(),
        secrets: HashMap::new(),
        tasks: vec![
            rustypipe::pipeline::parser::TaskDef {
                name: "task1".to_string(),
                depends_on: vec!["nonexistent".to_string()],
                run: "echo hello".to_string(),
                retries: None,
                timeout: None,
                backend: None,
                cache_key: None,
                continue_on_fail: None,
                when: None,
                before: None,
                after: None,
                on_failure: None,
            },
        ],
    };

    let result = validate_pipeline(&pipeline);
    assert!(result.is_err());
}

#[test]
fn test_validate_pipeline_detects_duplicate_names() {
    let pipeline = Pipeline {
        name: Some("test".to_string()),
        concurrency: Some(2),
        stop_on_fail: Some(false),
        backends: HashMap::new(),
        secrets: HashMap::new(),
        tasks: vec![
            rustypipe::pipeline::parser::TaskDef {
                name: "task1".to_string(),
                depends_on: vec![],
                run: "echo hello".to_string(),
                retries: None,
                timeout: None,
                backend: None,
                cache_key: None,
                continue_on_fail: None,
                when: None,
                before: None,
                after: None,
                on_failure: None,
            },
            rustypipe::pipeline::parser::TaskDef {
                name: "task1".to_string(),
                depends_on: vec![],
                run: "echo world".to_string(),
                retries: None,
                timeout: None,
                backend: None,
                cache_key: None,
                continue_on_fail: None,
                when: None,
                before: None,
                after: None,
                on_failure: None,
            },
        ],
    };

    let result = validate_pipeline(&pipeline);
    assert!(result.is_err());
}
