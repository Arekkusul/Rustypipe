use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "rustypipe")]
#[command(author, version, about = "A lightweight, dependency-aware pipeline executor", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run a pipeline from a YAML file
    Run {
        /// Path to the pipeline YAML file
        #[arg(value_name = "PIPELINE_FILE")]
        path: PathBuf,

        /// Override concurrency limit
        #[arg(short, long)]
        concurrency: Option<usize>,

        /// Stop on first failure (overrides pipeline config)
        #[arg(long)]
        stop_on_fail: bool,

        /// Continue on all failures (overrides pipeline config)
        #[arg(long, conflicts_with = "stop_on_fail")]
        continue_on_fail: bool,

        /// Dry run mode - show what would be executed without running tasks
        #[arg(long)]
        dry_run: bool,
    },

    /// Validate a pipeline YAML file without running it
    Validate {
        /// Path to the pipeline YAML file
        #[arg(value_name = "PIPELINE_FILE")]
        path: PathBuf,
    },

    /// List all pipeline runs
    History {
        /// Limit number of runs to show
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,
    },

    /// Show details of a specific run
    Show {
        /// Run ID to show details for
        #[arg(value_name = "RUN_ID")]
        run_id: String,
    },

    /// Clear the cache
    ClearCache,

    /// Clean up old pipeline runs
    Clean {
        /// Keep only the N most recent runs
        #[arg(short, long, default_value = "10")]
        keep: usize,
    },
}

pub fn get_cli() -> Cli {
    Cli::parse()
}
