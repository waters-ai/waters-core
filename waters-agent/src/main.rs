mod config;
mod engine;
mod subagent;
mod llm;
mod mcp;
mod autonomy;
mod skill;

#[cfg(feature = "kafka-transport")]
mod kafka;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "waters-agent", version, about = "WATERS Agent Runtime — standalone agent")]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,

    #[arg(short, long)]
    verbose: bool,

    #[cfg(feature = "kafka-transport")]
    #[arg(long)]
    kafka: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(
                    if args.verbose { "debug" } else { "info" }
                )),
        )
        .init();

    info!("WATERS Agent Runtime v{}", env!("CARGO_PKG_VERSION"));

    let cfg = if args.config.exists() {
        config::Config::from_file(&args.config)?
    } else {
        info!("No config found, using defaults");
        config::Config::default()
    };

    let mut engine = engine::AgentEngine::new(&cfg.agent.id, &cfg.agent.mission_id);

    #[cfg(feature = "kafka-transport")]
    if args.kafka {
        info!("Kafka transport enabled");
        engine.enable_kafka(&cfg.kafka).await?;
    }

    let mut autonomy = autonomy::AutonomyEngine::new();
    let start_time = std::time::Instant::now();

    info!("Agent {} on mission {} started", engine.agent_id(), engine.mission_id());
    info!("Autonomy: {} | Transport: {}",
        autonomy.current(),
        if engine.kafka_enabled() { "Kafka" } else { "Standalone" }
    );

    loop {
        engine.tick(&mut autonomy, start_time).await;
    }
}
