use core::config;
use std::{env, fs, path::PathBuf, process::exit, sync::Arc};

use anyhow::anyhow;
use const_format::formatcp;
use spdlog::{
    prelude::*,
    sink::{RotatingFileSink, RotationPolicy},
};

#[tokio::main]
async fn main() {
    let log_dir = setup_logger();

    info!("{} startup!", config::PACKAGE.name);
    info!("current version: {}", config::PACKAGE.version);
    info!("logs will be written to '{}'", log_dir.display());

    if let Err(err) = run().await {
        error!("exited with err: {err}");
        exit(1);
    }
}

fn setup_logger() -> PathBuf {
    if cfg!(debug_assertions) {
        spdlog::default_logger().set_level_filter(LevelFilter::All)
    }

    let log_dir = dirs::home_dir()
        .expect("no home directory found")
        .join(formatcp!(".{}/logs", config::PACKAGE.name));

    fs::create_dir_all(&log_dir).expect("failed to create log directory");

    let file_sink = Arc::new(
        RotatingFileSink::builder()
            .base_path(log_dir.join("log.txt"))
            .rotation_policy(RotationPolicy::Daily { hour: 0, minute: 0 })
            .build()
            .expect("failed to build log file sink"),
    );

    let logger = spdlog::default_logger()
        .fork_with(|logger| {
            logger.sinks_mut().push(file_sink);
            Ok(())
        })
        .expect("failed to build logger");

    spdlog::set_default_logger(logger);

    log_dir
}

async fn run() -> anyhow::Result<()> {
    let bot_token = env::var(config::BOT_TOKEN_ENV_VAR).map_err(|err| {
        anyhow!(
            "failed to read bot token from env var `{}`. err: '{err}'",
            config::BOT_TOKEN_ENV_VAR
        )
    })?;

    let db_url = env::var(config::DB_URL_ENV_VAR).map_err(|err| {
        anyhow!(
            "failed to read database url from env var `{}`. err: '{err}'",
            config::DB_URL_ENV_VAR
        )
    })?;

    core::run(bot_token, db_url).await
}
