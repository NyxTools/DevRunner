mod models;
mod scanner;
mod events;
mod process;
mod ui;
mod app;
mod cli;
mod config;

use anyhow::Result;
use clap::Parser;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    // Resolve target directory
    let target_dir = if args.path.is_absolute() {
        args.path
    } else {
        env::current_dir()?.join(args.path)
    };

    let _config = config::load_config(args.config, &target_dir)?;

    let services = scanner::scan_directory(&target_dir)?;

    app::run_app(services).await?;

    Ok(())
}
