mod auth;
#[cfg(debug_assertions)]
mod debug;
mod ping;
mod post;

use std::{borrow::Cow, sync::Arc};

use teloxide::{prelude::*, types::Me};

use crate::{cmd::Command, InstanceState};

pub struct Request {
    state: Arc<InstanceState>,
    bot: Bot,
    me: Me,
    msg: Message,
    cmd: Command,
}

impl Request {
    pub fn new(state: Arc<InstanceState>, bot: Bot, me: Me, msg: Message, cmd: Command) -> Self {
        Self {
            state,
            bot,
            me,
            msg,
            cmd,
        }
    }
}

pub enum Response<'a> {
    Nothing,
    ReplyTo(Cow<'a, str>),
    NewMsg(Cow<'a, str>),
}

pub async fn handle(req: Request) -> Result<(), teloxide::RequestError> {
    let req = &req;
    let chat_id = req.msg.chat.id;

    let res = handle_inner(req).await;
    let (succeeded, Ok(resp) | Err(resp)) = (res.is_ok(), res);

    let reply = |text, reply_to_msg_id| async move {
        let mut req = if succeeded {
            req.bot.send_message(chat_id, text)
        } else {
            req.bot.send_message(chat_id, format!("⚠️ {text}"))
        };
        if let Some(reply_to_msg_id) = reply_to_msg_id {
            req = req.reply_to_message_id(reply_to_msg_id);
        }
        req.await
    };

    match resp {
        Response::Nothing => return Ok(()),
        Response::ReplyTo(text) => reply(text, Some(req.msg.id)),
        Response::NewMsg(text) => reply(text, None),
    }
    .await?;

    Ok(())
}

async fn handle_inner(req: &Request) -> Result<Response<'_>, Response<'_>> {
    match &req.cmd {
        Command::Ping => ping::handle(req).await,
        #[cfg(debug_assertions)]
        Command::Debug(arg) => debug::handle(req, arg).await,
        Command::Auth(arg) => auth::auth(req, arg).await,
        Command::Revoke => auth::revoke(req).await,
        Command::Post => post::handle(req).await,
    }
}
