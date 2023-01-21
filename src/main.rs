use core::config;
use std::{env, process::exit};

use anyhow::anyhow;
use spdlog::prelude::*;

#[tokio::main]
async fn main() {
    setup_logger();

    info!("{} startup!", config::PACKAGE.name);
    info!("current version: {}", config::PACKAGE.version);

    if let Err(err) = run().await {
        error!("exited with err: {err}");
        exit(1);
    }
}

fn setup_logger() {
    if cfg!(debug_assertions) {
        spdlog::default_logger().set_level_filter(LevelFilter::All)
    }
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
