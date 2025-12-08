mod cli;
mod util;
mod plugins;
mod backends;
mod backend_config;
mod cache;
mod pipeline;

use anyhow::Context;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use cli::Commands;
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::get_cli();

    // Initialize logging with optional verbose mode
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(filter)
        .init();

    match cli.command {
        Commands::Run { path, concurrency, stop_on_fail, continue_on_fail, dry_run } => {
            // TODO: Apply CLI overrides to pipeline config
            let _ = (concurrency, stop_on_fail, continue_on_fail); // Prevent unused warnings

            if dry_run {
                pipeline::dry_run_pipeline(&path).await.context("dry run failed")?;
            } else {
                pipeline::run_pipeline(&path).await.context("pipeline run failed")?;
            }
        }
        Commands::Validate { path } => {
            pipeline::validate_pipeline_file(&path)?;
        }
        Commands::History { limit } => {
            show_history(limit)?;
        }
        Commands::Show { run_id } => {
            show_run_details(&run_id)?;
        }
        Commands::ClearCache => {
            clear_cache()?;
        }
        Commands::Clean { keep } => {
            clean_old_runs(keep)?;
        }
    }

    Ok(())
}

fn show_history(limit: usize) -> anyhow::Result<()> {
    let runs_dir = Path::new(".rustypipe/runs");

    if !runs_dir.exists() {
        println!("No pipeline runs found.");
        return Ok(());
    }

    let mut runs: Vec<_> = std::fs::read_dir(runs_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    // Sort by modification time (newest first)
    runs.sort_by_key(|e| {
        std::cmp::Reverse(
            e.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        )
    });

    println!("Recent pipeline runs:\n");
    println!("{:<40} {:<20} {}", "Run ID", "Date", "Pipeline");
    println!("{}", "-".repeat(80));

    for entry in runs.iter().take(limit) {
        let run_id = entry.file_name().to_string_lossy().to_string();
        let meta_file = entry.path().join("pipeline.yaml");

        let pipeline_name = if meta_file.exists() {
            std::fs::read_to_string(&meta_file)
                .ok()
                .and_then(|content| {
                    serde_yaml::from_str::<serde_yaml::Value>(&content)
                        .ok()
                        .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                })
                .unwrap_or_else(|| "<unnamed>".to_string())
        } else {
            "<unknown>".to_string()
        };

        let date = entry.metadata()
            .and_then(|m| m.modified())
            .ok()
            .map(|t| {
                let datetime: chrono::DateTime<chrono::Utc> = t.into();
                datetime.format("%Y-%m-%d %H:%M:%S").to_string()
            })
            .unwrap_or_else(|| "unknown".to_string());

        println!("{:<40} {:<20} {}", run_id, date, pipeline_name);
    }

    Ok(())
}

fn show_run_details(run_id: &str) -> anyhow::Result<()> {
    let run_dir = Path::new(".rustypipe/runs").join(run_id);

    if !run_dir.exists() {
        anyhow::bail!("Run ID '{}' not found", run_id);
    }

    println!("Run ID: {}\n", run_id);

    // Show pipeline config
    let meta_file = run_dir.join("pipeline.yaml");
    if meta_file.exists() {
        let content = std::fs::read_to_string(&meta_file)?;
        println!("Pipeline Configuration:");
        println!("{}", content);
        println!();
    }

    // List all task logs
    let mut logs: Vec<_> = std::fs::read_dir(&run_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s == "log")
                .unwrap_or(false)
        })
        .collect();

    logs.sort_by_key(|e| {
        e.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });

    println!("Task Logs:");
    println!("{}", "-".repeat(80));
    for log in logs {
        let log_name = log.file_name().to_string_lossy().to_string();
        let log_path = log.path();

        if let Ok(content) = std::fs::read_to_string(&log_path) {
            println!("\n{}", log_name);
            println!("{}", content);
        }
    }

    Ok(())
}

fn clear_cache() -> anyhow::Result<()> {
    let cache_dir = Path::new(".rustypipe/cache");

    if cache_dir.exists() {
        std::fs::remove_dir_all(cache_dir)?;
        println!("Cache cleared successfully.");
    } else {
        println!("No cache found.");
    }

    Ok(())
}

fn clean_old_runs(keep: usize) -> anyhow::Result<()> {
    let runs_dir = Path::new(".rustypipe/runs");

    if !runs_dir.exists() {
        println!("No pipeline runs found.");
        return Ok(());
    }

    let mut runs: Vec<_> = std::fs::read_dir(runs_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    // Sort by modification time (newest first)
    runs.sort_by_key(|e| {
        std::cmp::Reverse(
            e.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        )
    });

    let total_runs = runs.len();
    if total_runs <= keep {
        println!("No runs to clean (found {} runs, keeping {})", total_runs, keep);
        return Ok(());
    }

    let to_remove = &runs[keep..];
    let removed_count = to_remove.len();

    for entry in to_remove {
        if let Err(e) = std::fs::remove_dir_all(entry.path()) {
            eprintln!("Failed to remove run {}: {}", entry.file_name().to_string_lossy(), e);
        }
    }

    println!("Cleaned {} old run(s), kept {} recent run(s)", removed_count, keep);

    Ok(())
}
