use crate::pipeline::parser::{TaskDef, load_pipeline, validate_pipeline};
use crate::util::{create_run_dir, interpolate_command, write_artifact, timestamp};
use crate::backends::Backend;
use crate::backend_config::BackendManager;
use crate::cache::Cache;
use futures::stream::{FuturesUnordered, StreamExt};
use futures::FutureExt;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore, Notify};
use serde_json::json;
use tracing::{info, warn};
use chrono::Utc;
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};

/// Public entry used by main.rs
pub async fn run_pipeline(path: &Path) -> Result<()> {
    let pipeline = load_pipeline(path)?;
    validate_pipeline(&pipeline)?;

    info!("Starting pipeline: {:?}", pipeline.name);

    // create run dir for artifacts
    let base = Path::new(".rustypipe");
    let run_dir = create_run_dir(base)?;
    let meta_file = run_dir.join("pipeline.yaml");
    std::fs::write(&meta_file, serde_yaml::to_string(&pipeline)?)?;

    // Initialize cache
    let cache = Arc::new(Cache::new(base)?);

    // Initialize backend manager
    let backend_manager = Arc::new(
        BackendManager::new(pipeline.backends.clone())
            .context("failed to initialize backend manager")?
    );

    // Build graph structures
    let mut tasks_map: HashMap<String, TaskDef> = HashMap::new();
    let mut indegree: HashMap<String, usize> = HashMap::new();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();

    for t in pipeline.tasks.into_iter() {
        indegree.entry(t.name.clone()).or_insert(0);
        for dep in &t.depends_on {
            adj.entry(dep.clone()).or_default().push(t.name.clone());
            *indegree.entry(t.name.clone()).or_insert(0) += 1;
        }
        tasks_map.insert(t.name.clone(), t);
    }

    // concurrency & stop_on_fail
    let concurrency = pipeline.concurrency.unwrap_or(4);
    let stop_on_fail = pipeline.stop_on_fail.unwrap_or(false);
    let pipeline_dir = path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();

    // shared state for interpolation & task outputs
    let outputs = Arc::new(Mutex::new(HashMap::<String,String>::new()));
    let secrets = Arc::new(pipeline.secrets.clone());
    let vars = Arc::new(Mutex::new(HashMap::<String,String>::new()));

    // Progress tracking
    let multi_progress = MultiProgress::new();
    let total_tasks = tasks_map.len();
    let overall_progress = multi_progress.add(ProgressBar::new(total_tasks as u64));
    overall_progress.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} tasks")
            .expect("invalid progress template")
            .progress_chars("#>-")
    );
    overall_progress.set_message("Pipeline progress");

    let completed_tasks = Arc::new(Mutex::new(0usize));

    // concurrency control
    let sem = Arc::new(Semaphore::new(concurrency));

    // initial ready tasks
    let mut ready_tasks: Vec<String> = indegree.iter()
        .filter_map(|(n,&d)| if d==0 { Some(n.clone()) } else { None })
        .collect();

    let mut running = FuturesUnordered::new();
    // spawn initial batch
    for t in ready_tasks.drain(..) {
        let task_progress = multi_progress.add(ProgressBar::new_spinner());
        task_progress.set_message(format!("Task: {}", t));
        running.push(spawn_task_future(
            t,
            pipeline_dir.clone(),
            tasks_map.clone(),
            run_dir.clone(),
            outputs.clone(),
            vars.clone(),
            secrets.clone(),
            backend_manager.clone(),
            cache.clone(),
            sem.clone(),
            Some(task_progress),
        ));
    }

    let mut current_indegree = indegree;
    let mut ordered_results: Vec<(String, String, String, String, bool)> = Vec::new(); // task, cmd, stdout, stderr, success
    let mut has_failures = false;

    // graceful shutdown notify
    let shutdown_notify = Arc::new(Notify::new());
    {
        let shutdown_notify = shutdown_notify.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            eprintln!("Received Ctrl+C — initiating shutdown");
            shutdown_notify.notify_one();
        });
    }

    // driver loop: process completed tasks and spawn dependents
    while let Some(res) = running.next().await {
        if shutdown_notify.notified().now_or_never().is_some() {
            eprintln!("Shutdown requested; stopping spawn of new tasks.");
            break;
        }

        match res {
            Ok((task_name, cmd, stdout, stderr, exit_status, from_cache)) => {
                let success = exit_status.success();

                // Save artifacts (skip if from cache)
                if !from_cache {
                    let ts = timestamp();
                    let safe_task_name = sanitize_filename(&task_name);
                    let log_name = format!("{}_{}.log", safe_task_name, ts);
                    let meta_name = format!("{}_{}.json", safe_task_name, ts);

                    let summary = format!("Task: {}\nCmd: {}\nExit: {:?}\nStdout:\n{}\nStderr:\n{}\n",
                        task_name, cmd, exit_status.code(), stdout, stderr);
                    write_artifact(&run_dir, &log_name, &summary)?;

                    let meta = json!({
                        "task": task_name,
                        "command": cmd,
                        "exit_code": exit_status.code(),
                        "timestamp": Utc::now().to_rfc3339(),
                    });
                    write_artifact(&run_dir, &meta_name, &meta.to_string())?;
                }

                // store output for interpolation
                {
                    let mut out_map = outputs.lock().await;
                    out_map.insert(task_name.clone(), stdout.clone());
                }

                ordered_results.push((task_name.clone(), cmd.clone(), stdout.clone(), stderr.clone(), success));

                // Update progress
                {
                    let mut completed = completed_tasks.lock().await;
                    *completed += 1;
                    overall_progress.set_position(*completed as u64);
                }

                // Get task definition to check continue_on_fail
                let task_def = tasks_map.get(&task_name).cloned();
                let continue_on_fail = task_def.as_ref()
                    .and_then(|t| t.continue_on_fail)
                    .unwrap_or(false);

                // fail-fast behavior
                if !success {
                    has_failures = true;
                    if !continue_on_fail && stop_on_fail {
                        overall_progress.finish_with_message("Pipeline aborted due to failure");
                        anyhow::bail!("Task '{}' failed (code {:?}); aborting (stop_on_fail=true)", task_name, exit_status.code());
                    } else if !continue_on_fail {
                        warn!("Task '{}' failed but continuing (stop_on_fail=false)", task_name);
                    } else {
                        warn!("Task '{}' failed but continuing (continue_on_fail=true)", task_name);
                    }
                }

                // spawn dependents whose indegree drops to 0
                if let Some(dependents) = adj.get(&task_name) {
                    for dep in dependents {
                        if let Some(val) = current_indegree.get_mut(dep) {
                            *val = val.saturating_sub(1);
                            if *val == 0 {
                                let task_progress = multi_progress.add(ProgressBar::new_spinner());
                                task_progress.set_message(format!("Task: {}", dep));
                                running.push(spawn_task_future(
                                    dep.clone(),
                                    pipeline_dir.clone(),
                                    tasks_map.clone(),
                                    run_dir.clone(),
                                    outputs.clone(),
                                    vars.clone(),
                                    secrets.clone(),
                                    backend_manager.clone(),
                                    cache.clone(),
                                    sem.clone(),
                                    Some(task_progress),
                                ));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                has_failures = true;
                eprintln!("Task future failed: {:?}", e);
                if stop_on_fail {
                    overall_progress.finish_with_message("Pipeline aborted due to error");
                    anyhow::bail!("A task future failed: {:?}", e);
                }
            }
        }
    }

    overall_progress.finish_with_message(if has_failures { "Pipeline completed with failures" } else { "Pipeline completed successfully" });

    // print ordered results
    println!("\n=== Pipeline Results ===\n");
    for (task, cmd, stdout, stderr, success) in ordered_results {
        let status_icon = if success { "✓" } else { "✗" };
        println!("{} Task: {}", status_icon, task);
        println!("Command: {}", cmd);
        println!("Output: {}", stdout.trim());
        if !stderr.trim().is_empty() {
            eprintln!("Error: {}", stderr.trim());
        }
        println!();
    }

    if has_failures {
        warn!("Pipeline finished with failures");
        anyhow::bail!("Pipeline completed with one or more failures");
    } else {
        info!("Pipeline finished successfully");
    }

    Ok(())
}

/// Validate-only helper for main.rs
pub fn validate_pipeline_file(path: &Path) -> Result<()> {
    let pipeline = load_pipeline(path)?;
    validate_pipeline(&pipeline)?;
    println!("Pipeline '{}' validated successfully",
        pipeline.name.unwrap_or_else(|| "<unnamed>".to_string()));
    Ok(())
}

/// Spawn a future for a single task; returns a future that resolves to (name, cmd, stdout, stderr, exit_status, from_cache)
async fn spawn_task_future(
    task_name: String,
    pipeline_dir: PathBuf,
    tasks_map: HashMap<String, TaskDef>,
    _run_dir: PathBuf,
    outputs: Arc<Mutex<HashMap<String,String>>>,
    vars: Arc<Mutex<HashMap<String,String>>>,
    secrets: Arc<HashMap<String, String>>,
    backend_manager: Arc<BackendManager>,
    cache: Arc<Cache>,
    sem: Arc<Semaphore>,
    progress: Option<ProgressBar>,
) -> Result<(String, String, String, String, std::process::ExitStatus, bool)> {
    let _permit = sem.acquire().await;

    let task_def = tasks_map.get(&task_name)
        .with_context(|| format!("task '{}' not found in tasks map", task_name))?
        .clone();

    let retries = task_def.retries.unwrap_or(0);
    let timeout_secs = task_def.timeout;

    if let Some(ref pb) = progress {
        pb.set_message(format!("Running task: {}", task_name));
    }

    // Get backend
    let backend_name = task_def.backend.as_deref().unwrap_or("local");
    let backend = backend_manager.get(backend_name);

    // Interpolate command with outputs, vars, and secrets
    let outputs_snapshot = outputs.lock().await.clone();
    let vars_snapshot = vars.lock().await.clone();

    // Merge secrets into vars for interpolation (secrets take precedence)
    let mut combined_vars = vars_snapshot.clone();
    for (k, v) in secrets.as_ref() {
        combined_vars.insert(k.clone(), v.clone());
    }

    let cmd = interpolate_command(&task_def.run, &outputs_snapshot, &combined_vars);

    // Check cache if cache_key is present
    if let Some(cache_key_template) = &task_def.cache_key {
        let cache_key = interpolate_command(cache_key_template, &outputs_snapshot, &combined_vars);

        if let Ok(Some((cached_stdout, cached_stderr, cached_exit_code))) = cache.get(&cache_key) {
            info!("Task '{}' result found in cache", task_name);
            if let Some(ref pb) = progress {
                pb.finish_with_message(format!("✓ {} (cached)", task_name));
            }

            // Create a fake exit status
            #[cfg(unix)]
            let status = {
                use std::os::unix::process::ExitStatusExt;
                std::process::ExitStatus::from_raw(cached_exit_code.unwrap_or(0) << 8)
            };

            #[cfg(windows)]
            let status = {
                use std::os::windows::process::ExitStatusExt;
                std::process::ExitStatus::from_raw(cached_exit_code.unwrap_or(0) as u32)
            };

            return Ok((task_name, cmd, cached_stdout, cached_stderr, status, true));
        }
    }

    // Run task with retries
    let mut attempt = 0u32;
    let result = loop {
        attempt += 1;

        if let Some(ref pb) = progress {
            if attempt > 1 {
                pb.set_message(format!("Retrying task: {} (attempt {})", task_name, attempt));
            }
        }

        let run_result = backend.run(&cmd, &pipeline_dir, timeout_secs).await;

        match run_result {
            Ok((stdout, stderr, status)) => {
                break Ok((stdout, stderr, status));
            }
            Err(e) => {
                if attempt <= retries {
                    warn!("Task '{}' attempt {} failed: {:?}. Retrying...", task_name, attempt, e);
                    tokio::time::sleep(std::time::Duration::from_secs(2_u64.pow(attempt.min(5)))).await;
                    continue;
                } else {
                    break Err(e);
                }
            }
        }
    };

    let (stdout, stderr, status) = result?;

    // Cache result if cache_key is present
    if let Some(cache_key_template) = &task_def.cache_key {
        let cache_key = interpolate_command(cache_key_template, &outputs_snapshot, &combined_vars);
        if let Err(e) = cache.set(&cache_key, &stdout, &stderr, status.code()) {
            warn!("Failed to cache result for task '{}': {:?}", task_name, e);
        }
    }

    if let Some(ref pb) = progress {
        let icon = if status.success() { "✓" } else { "✗" };
        pb.finish_with_message(format!("{} {}", icon, task_name));
    }

    Ok((task_name, cmd, stdout, stderr, status, false))
}

/// Replace illegal Windows filename characters
fn sanitize_filename(name: &str) -> String {
    let illegal = ['<','>','/','\\','|','?','*',':','"'];
    name.chars()
        .map(|c| if illegal.contains(&c) { '_' } else { c })
        .collect()
}
