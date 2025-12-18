mod models;
mod scanner;
mod events;
mod process;
mod ui;
mod app;

use anyhow::Result;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Scan for services
    let current_dir = env::current_dir()?;
    let services = scanner::scan_directory(&current_dir)?;

    // 2. Start App
    app::run_app(services).await?;

    Ok(())
}
