mod auth;
#[cfg(debug_assertions)]
mod debug;
mod ping;
mod post;
mod start;

use std::{borrow::Cow, sync::Arc};

use spdlog::prelude::*;
use teloxide::{
    prelude::*,
    types::{ChatKind, Me},
};

use crate::{
    cmd::Command,
    util::{media, ProgMsg},
    InstanceState,
};

struct RequestMeta {
    state: Arc<InstanceState>,
    bot: Bot,
    me: Me,
    msg: Message,
}

enum RequestKind {
    NewMessage,
    EditedMessage,
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

    pub fn edited_message(state: Arc<InstanceState>, bot: Bot, me: Me, msg: Message) -> Self {
        Self {
            meta: RequestMeta {
                state,
                bot,
                me,
                msg,
            },
            kind: RequestKind::EditedMessage,
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

pub enum ResponseKind<'a> {
    Nothing,
    ReplyTo(Cow<'a, str>),
    NewMsg(Cow<'a, str>),
}

pub struct Response<'a> {
    kind: ResponseKind<'a>,
    disable_preview: bool,
}

impl<'a> Response<'a> {
    pub fn nothing() -> Self {
        Self {
            kind: ResponseKind::Nothing,
            disable_preview: false,
        }
    }

    pub fn reply_to(text: impl Into<Cow<'a, str>>) -> Self {
        Self {
            kind: ResponseKind::ReplyTo(text.into()),
            disable_preview: false,
        }
    }

    pub fn new_msg(text: impl Into<Cow<'a, str>>) -> Self {
        Self {
            kind: ResponseKind::NewMsg(text.into()),
            disable_preview: false,
        }
    }

    pub fn disable_preview(mut self) -> Self {
        self.disable_preview = true;
        self
    }
}

pub async fn handle(req: Request) -> Result<(), teloxide::RequestError> {
    let req = &req;
    let chat_id = req.meta.msg.chat.id;

    let res = handle_kind(req).await;
    let (succeeded, Ok(resp) | Err(resp)) = (res.is_ok(), res);

    let reply = |text, reply_to_msg_id, disable_preview: bool| async move {
        let mut req = if succeeded {
            req.meta.bot.send_message(chat_id, text)
        } else {
            req.meta.bot.send_message(chat_id, format!("⚠️ {text}"))
        };
        if let Some(reply_to_msg_id) = reply_to_msg_id {
            req = req.reply_to_message_id(reply_to_msg_id);
        }
        if disable_preview {
            req = req.disable_web_page_preview(disable_preview);
        }
        req.await
    };

    match resp.kind {
        ResponseKind::Nothing => return Ok(()),
        ResponseKind::ReplyTo(text) => reply(text, Some(req.meta.msg.id), resp.disable_preview),
        ResponseKind::NewMsg(text) => reply(text, None, resp.disable_preview),
    }
    .await?;

    Ok(())
}

async fn handle_kind(req: &Request) -> Result<Response<'_>, Response<'_>> {
    match &req.kind {
        RequestKind::NewMessage => handle_new_message(req).await,
        RequestKind::EditedMessage => handle_edited_message(req).await,
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

    media::on_new_or_edited_message(state, msg).await;
    Ok(Response::nothing())
}

async fn handle_edited_message(req: &Request) -> Result<Response<'_>, Response<'_>> {
    let (state, msg) = (&req.meta.state, &req.meta.msg);

    trace!(
        "edited message. chat id '{}', msg id '{}'",
        msg.chat.id,
        msg.id
    );

    media::on_new_or_edited_message(state, msg).await;
    Ok(Response::nothing())
}

async fn handle_command<'a>(
    req: &'a Request,
    cmd: &'a Command,
) -> Result<Response<'a>, Response<'a>> {
    match cmd {
        Command::Ping => ping::handle(req).await,
        #[cfg(debug_assertions)]
        Command::Debug(arg) => debug::handle(req, arg).await,
        Command::Start => start::handle(req).await,
        Command::Auth(arg) => {
            require_private(req)?;
            auth::auth(req, arg).await
        }
        Command::Revoke => {
            require_private(req)?;
            auth::revoke(req).await
        }
        Command::Post(arg) => {
            let mut prog_msg = ProgMsg::new(&req.meta.bot, &req.meta.msg, "Synchronizing...");
            let result = post::handle(req, &mut prog_msg, arg).await;
            result
        }
    }
}

fn require_private(req: &Request) -> Result<(), Response<'_>> {
    match req.meta.msg.chat.kind {
        ChatKind::Private(_) => Ok(()),
        ChatKind::Public(_) => Err(Response::reply_to(
            "This command is only available in direct messages.",
        )),
    }
}
