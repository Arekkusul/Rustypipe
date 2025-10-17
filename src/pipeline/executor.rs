use crate::pipeline::parser::{TaskDef, load_pipeline, validate_pipeline};
use crate::util::{create_run_dir, interpolate_command, write_artifact, timestamp};
use crate::backends::{Backend, LocalBackend};
use futures::stream::{FuturesUnordered, StreamExt};
use futures::FutureExt;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore, Notify};
use serde_json::json;
use tracing::info;
use chrono::Utc;

/// Public entry used by main.rs
pub async fn run_pipeline(path: &Path) -> anyhow::Result<()> {
    let pipeline = load_pipeline(path)?;
    validate_pipeline(&pipeline)?;

    info!("Starting pipeline: {:?}", pipeline.name);

    // create run dir for artifacts
    let base = Path::new(".rustypipe");
    let run_dir = create_run_dir(base)?;
    let meta_file = run_dir.join("pipeline.yaml");
    std::fs::write(&meta_file, serde_yaml::to_string(&pipeline)?)?;

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
    let vars = Arc::new(Mutex::new(HashMap::<String,String>::new()));

    // backend resolver (only LocalBackend implemented)
    let local_backend = Arc::new(LocalBackend::new());

    // concurrency control
    let sem = Arc::new(Semaphore::new(concurrency));

    // initial ready tasks
    let mut ready_tasks: Vec<String> = indegree.iter()
        .filter_map(|(n,&d)| if d==0 { Some(n.clone()) } else { None })
        .collect();

    let mut running = FuturesUnordered::new();
    // spawn initial batch
    for t in ready_tasks.drain(..) {
        running.push(spawn_task_future(
            t,
            pipeline_dir.clone(),
            tasks_map.clone(),
            run_dir.clone(),
            outputs.clone(),
            vars.clone(),
            local_backend.clone(),
            sem.clone(),
        ));
    }

    let mut current_indegree = indegree;
    let mut ordered_results: Vec<(String, String, String, String)> = Vec::new(); // task, cmd, stdout, stderr

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
            Ok((task_name, cmd, stdout, stderr, exit_status)) => {
                // Save artifacts
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

                // store output for interpolation
                {
                    let mut out_map = outputs.lock().await;
                    out_map.insert(task_name.clone(), stdout.clone());
                }

                ordered_results.push((task_name.clone(), cmd.clone(), stdout.clone(), stderr.clone()));

                // fail-fast behavior
                if !exit_status.success() && stop_on_fail {
                    anyhow::bail!("Task '{}' failed (code {:?}); aborting (stop_on_fail=true)", task_name, exit_status.code());
                }

                // spawn dependents whose indegree drops to 0
                if let Some(dependents) = adj.get(&task_name) {
                    for dep in dependents {
                        if let Some(val) = current_indegree.get_mut(dep) {
                            *val = val.saturating_sub(1);
                            if *val == 0 {
                                running.push(spawn_task_future(
                                    dep.clone(),
                                    pipeline_dir.clone(),
                                    tasks_map.clone(),
                                    run_dir.clone(),
                                    outputs.clone(),
                                    vars.clone(),
                                    local_backend.clone(),
                                    sem.clone(),
                                ));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Task future failed: {:?}", e);
                if stop_on_fail {
                    anyhow::bail!("A task future failed: {:?}", e);
                }
            }
        }
    }

    // print ordered results
    for (task, cmd, stdout, stderr) in ordered_results {
        println!("Task: {}", task);
        println!("Command: {}", cmd);
        println!("Output: {}", stdout.trim());
        if !stderr.trim().is_empty() {
            eprintln!("Error: {}", stderr.trim());
        }
        println!();
    }

    info!("Pipeline finished");
    Ok(())
}

/// Validate-only helper for main.rs
pub fn validate_pipeline_file(path: &Path) -> anyhow::Result<()> {
    let pipeline = load_pipeline(path)?;
    validate_pipeline(&pipeline)?;
    println!("Pipeline validated");
    Ok(())
}

/// Spawn a future for a single task; returns a future that resolves to (name, cmd, stdout, stderr, exit_status)
async fn spawn_task_future(
    task_name: String,
    pipeline_dir: PathBuf,
    tasks_map: HashMap<String, TaskDef>,
    _run_dir: PathBuf,
    outputs: Arc<Mutex<HashMap<String,String>>>,
    vars: Arc<Mutex<HashMap<String,String>>>,
    backend: Arc<dyn Backend>,
    sem: Arc<Semaphore>,
) -> anyhow::Result<(String, String, String, String, std::process::ExitStatus)> {
    let _permit = sem.acquire().await;

    let task_def = tasks_map.get(&task_name).expect("task exists").clone();
    let retries = task_def.retries.unwrap_or(0);
    let timeout_secs = task_def.timeout;

    let backend_name = task_def.backend.clone().unwrap_or_else(|| "local".to_string());
    let backend: Arc<dyn Backend> = match backend_name.as_str() {
        "local" => backend.clone(),
        _ => backend.clone(),
    };

    let outputs_snapshot = outputs.lock().await.clone();
    let vars_snapshot = vars.lock().await.clone();
    let cmd = interpolate_command(&task_def.run, &outputs_snapshot, &vars_snapshot);

    let mut attempt = 0u32;
    loop {
        attempt += 1;
        let run_result = backend.run(&cmd, &pipeline_dir, timeout_secs).await;

        match run_result {
            Ok((stdout, stderr, status)) => {
                return Ok((task_name, cmd, stdout, stderr, status));
            }
            Err(e) => {
                if attempt <= retries {
                    eprintln!("Task '{}' attempt {} failed: {:?}. Retrying...", task_def.name, attempt, e);
                    continue;
                } else {
                    return Err(e);
                }
            }
        }
    }
}

/// Replace illegal Windows filename characters
fn sanitize_filename(name: &str) -> String {
    let illegal = ['<','>','/','\\','|','?','*',':','"'];
    name.chars()
        .map(|c| if illegal.contains(&c) { '_' } else { c })
        .collect()
}
