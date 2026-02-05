use clap::Parser;
use rust_core::{AudioEngine, ipc::server::WebSocketServer, config::manager::ConfigManager};
use tracing::info;
use tracing_subscriber;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rust_core=debug,info".into())
        )
        .init();

    let args = Args::parse();

    let config_manager = ConfigManager::new(args.config)?;
    let config = config_manager.load()?;

    info!("Starting Roobar3000 Audio Engine...");

    let mut audio_engine = AudioEngine::new(config.audio.clone())?;
    audio_engine.start()?;

    let ws_server = WebSocketServer::new(
        "127.0.0.1:8080",
        audio_engine.command_sender().clone(),
        audio_engine.player().clone()
    )?;
    ws_server.start().await?;

    Ok(())
}
