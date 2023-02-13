mod cmd;
pub mod config;
mod db;
mod handler;
mod mastodon;
mod util;

use std::sync::Arc;

use cmd::Command;
use spdlog::prelude::*;
use teloxide::{
    prelude::*,
    types::{Me, Update},
    utils::command::BotCommands,
};

use crate::util::handle;

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

    bot.set_my_commands(Command::bot_commands()).await?;

    let handler =
        dptree::entry()
            .branch(
                Update::filter_message()
                    .inspect_async(
                        |state: Arc<InstanceState>, bot: Bot, me: Me, msg: Message| async move {
                            let req = handle::Request::new_message(state, bot, me, msg);
                            _ = handler::handle(req).await;
                        },
                    )
                    .branch(dptree::entry().filter_command::<Command>().endpoint(
                        |state: Arc<InstanceState>,
                         bot: Bot,
                         me: Me,
                         msg: Message,
                         cmd: Command| async move {
                            let req = handle::Request::new_command(state, bot, me, msg, cmd);
                            handler::handle(req).await
                        },
                    )),
            )
            .branch(Update::filter_edited_message().inspect_async(
                |state: Arc<InstanceState>, bot: Bot, me: Me, msg: Message| async move {
                    let req = handle::Request::edited_message(state, bot, me, msg);
                    _ = handler::handle(req).await;
                },
            ));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![inst_state])
        .default_handler(|upd| async move {
            debug!("unhandled update: {upd:?}");
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
