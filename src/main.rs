mod cli;
mod util;
mod plugins;
mod backends;
mod pipeline;

use anyhow::Context;
use tracing_subscriber::{fmt, prelude::*};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .init();

    let opts = cli::get_opts();
    match opts.subcommand.as_str() {
        "run" => {
            let path = std::path::Path::new(&opts.path);
            pipeline::run_pipeline(path).await.context("pipeline run failed")?;
        }
        "validate" => {
            pipeline::validate_pipeline_file(std::path::Path::new(&opts.path))?;
        }
        other => {
            eprintln!("Unknown subcommand: {} (supported: run, validate)", other);
        }
    }

    Ok(())
}
