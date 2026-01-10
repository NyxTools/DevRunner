use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Optional path to scan for services
    #[arg(short, long, default_value = ".")]
    pub path: PathBuf,

    /// Optional path to a configuration file
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}
