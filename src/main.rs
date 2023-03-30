//! Antikoerper is a simple and lightweight data aggregation and visualization tool

use std::io::Read;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use log::{error, info};

mod app;
mod conf;
mod item;
mod output;

#[derive(Parser)]
#[command(name = "Antikörper")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "CONFIG")]
    config: Option<PathBuf>,
    #[arg(short, long)]
    daemonize: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::from_default_env().init();

    let config_path = cli
        .config
        .unwrap_or_else(|| PathBuf::from("/etc/antikoerper/config.toml"));

    if cli.daemonize {
        let mut child = std::process::Command::new(
            std::env::args()
                .next()
                .expect("std::env::args had a length of zero!"),
        );
        let args = std::env::args()
            .skip(1)
            .filter(|arg| arg != "-d" && arg != "--daemonize")
            .collect::<Vec<_>>();
        child
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        child.spawn().map_err(|e| {
            error!("Failed daemonizing the process");
            error!("{}", e);
            e
        })?;
    }

    info!("Config file used: {}", &config_path.display());

    let mut file = std::fs::File::open(&config_path).map_err(|e| {
        error!("Failed opening configuration file, {}", e);
        e
    })?;

    let config = conf::load(&mut file as &mut dyn Read).map_err(|e| {
        error!("Failed parsing configuration, {}", e);
        e
    })?;

    let app = app::App::from(config);

    app.start().await.map_err(|e| {
        error!("Application startup failed for following reason:");
        error!("{}", e);
        e
    })
}
