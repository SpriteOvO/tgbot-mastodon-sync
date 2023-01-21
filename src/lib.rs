mod cmd;
pub mod config;
mod db;
mod handler;
mod mastodon;

use std::sync::Arc;

use spdlog::prelude::*;
use teloxide::{
    prelude::*,
    types::{Me, Update},
    utils::command::BotCommands,
};

pub struct InstanceState {
    pub db: db::Pool,
}

impl InstanceState {
    async fn new(db_url: impl AsRef<str>) -> anyhow::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            db: db::Pool::connect(db_url).await?,
        }))
    }
}

pub async fn run(bot_token: impl Into<String>, db_url: impl AsRef<str>) -> anyhow::Result<()> {
    let bot = Bot::new(bot_token);
    let inst_state = InstanceState::new(db_url).await?;

    bot.set_my_commands(cmd::Command::bot_commands()).await?;

    let handler = Update::filter_message()
        .branch(dptree::entry().filter_command::<cmd::Command>().endpoint(
        |state: Arc<InstanceState>, bot: Bot, me: Me, msg: Message, cmd: cmd::Command| async move {
            let req = handler::Request::new(state, bot, me, msg, cmd);
            handler::handle(req).await
        },
    ));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![inst_state])
        .default_handler(|upd| async move {
            trace!("unhandled update: {upd:?}");
        })
        .error_handler(Arc::new(
            |err| async move { error!("dispatcher error: {err}") },
        ))
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}