mod auth;
#[cfg(debug_assertions)]
mod debug;
mod ping;
mod post;

use std::{borrow::Cow, sync::Arc};

use spdlog::prelude::*;
use teloxide::{prelude::*, types::Me};

use crate::{cmd::Command, util::media, InstanceState};

struct RequestMeta {
    state: Arc<InstanceState>,
    bot: Bot,
    me: Me,
    msg: Message,
}

enum RequestKind {
    NewMessage,
    Command(Command),
}

pub struct Request {
    meta: RequestMeta,
    kind: RequestKind,
}

impl Request {
    pub fn new_message(state: Arc<InstanceState>, bot: Bot, me: Me, msg: Message) -> Self {
        Self {
            meta: RequestMeta {
                state,
                bot,
                me,
                msg,
            },
            kind: RequestKind::NewMessage,
        }
    }

    pub fn new_command(
        state: Arc<InstanceState>,
        bot: Bot,
        me: Me,
        msg: Message,
        cmd: Command,
    ) -> Self {
        Self {
            meta: RequestMeta {
                state,
                bot,
                me,
                msg,
            },
            kind: RequestKind::Command(cmd),
        }
    }
}

pub enum Response<'a> {
    Nothing,
    ReplyTo(Cow<'a, str>),
    NewMsg(Cow<'a, str>),
}

use Response::*;

pub async fn handle(req: Request) -> Result<(), teloxide::RequestError> {
    let req = &req;
    let chat_id = req.meta.msg.chat.id;

    let res = handle_kind(req).await;
    let (succeeded, Ok(resp) | Err(resp)) = (res.is_ok(), res);

    let reply = |text, reply_to_msg_id| async move {
        let mut req = if succeeded {
            req.meta.bot.send_message(chat_id, text)
        } else {
            req.meta.bot.send_message(chat_id, format!("⚠️ {text}"))
        };
        if let Some(reply_to_msg_id) = reply_to_msg_id {
            req = req.reply_to_message_id(reply_to_msg_id);
        }
        req.await
    };

    match resp {
        Nothing => return Ok(()),
        ReplyTo(text) => reply(text, Some(req.meta.msg.id)),
        NewMsg(text) => reply(text, None),
    }
    .await?;

    Ok(())
}

async fn handle_kind(req: &Request) -> Result<Response<'_>, Response<'_>> {
    match &req.kind {
        RequestKind::NewMessage => handle_new_message(req).await,
        RequestKind::Command(cmd) => handle_command(req, cmd).await,
    }
}

async fn handle_new_message(req: &Request) -> Result<Response<'_>, Response<'_>> {
    let (state, msg) = (&req.meta.state, &req.meta.msg);

    trace!(
        "new message. chat id '{}', msg id '{}'",
        msg.chat.id,
        msg.id
    );

    media::on_new_message(state, msg).await;
    Ok(Nothing)
}

async fn handle_command<'a>(
    req: &'a Request,
    cmd: &'a Command,
) -> Result<Response<'a>, Response<'a>> {
    match cmd {
        Command::Ping => ping::handle(req).await,
        #[cfg(debug_assertions)]
        Command::Debug(arg) => debug::handle(req, arg).await,
        Command::Auth(arg) => auth::auth(req, arg).await,
        Command::Revoke => auth::revoke(req).await,
        Command::Post => post::handle(req).await,
    }
}
