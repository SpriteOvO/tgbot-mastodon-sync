mod auth;
mod broadcast;
#[cfg(debug_assertions)]
mod debug;
mod ping;
mod post;
mod start;

use std::{env, sync::Arc};

use spdlog::prelude::*;
use teloxide::{payloads::SendMessageSetters, prelude::*, types::ChatKind};
use tgbot_utils::{
    handle::{self, RequestKind::*, Response, ResponseKind::*},
    media,
    text::*,
    ProgMsg,
};

use crate::{cmd::Command, config, InstanceState};

type Request = handle::Request<Arc<InstanceState>, Command>;

pub async fn handle(req: Request) -> Result<(), teloxide::RequestError> {
    let req = &req;
    let chat_id = req.msg().chat.id;

    let res = handle_kind(req).await;
    let (succeeded, Ok(resp) | Err(resp)) = (res.is_ok(), res);

    let reply = |mut text: MessageText<'_>, reply_to_msg_id| {
        if !succeeded {
            text.prepend("⚠️ ");
        }
        let mut req = text.executor(req.bot()).send_message(chat_id);
        if let Some(reply_to_msg_id) = reply_to_msg_id {
            req = req.reply_to_message_id(reply_to_msg_id);
        }
        req
    };

    match resp.kind {
        Nothing => return Ok(()),
        ReplyTo(text) => reply(text, Some(req.msg().id)).await,
        NewMsg(text) => reply(text, None).await,
    }?;

    Ok(())
}

async fn handle_kind(req: &Request) -> Result<Response<'_>, Response<'_>> {
    match req.kind() {
        NewMessage => handle_new_message(req).await,
        EditedMessage => handle_edited_message(req).await,
        Command(cmd) => handle_command(req, cmd).await,
    }
}

async fn handle_new_message(req: &Request) -> Result<Response<'_>, Response<'_>> {
    trace!(
        "new message. chat id '{}', msg id '{}'",
        req.msg().chat.id,
        req.msg().id
    );

    media::on_new_or_edited_message(|| req.state().db.pool(), req.msg()).await;
    Ok(Response::nothing())
}

async fn handle_edited_message(req: &Request) -> Result<Response<'_>, Response<'_>> {
    trace!(
        "edited message. chat id '{}', msg id '{}'",
        req.msg().chat.id,
        req.msg().id
    );

    media::on_new_or_edited_message(|| req.state().db.pool(), req.msg()).await;
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
            let mut prog_msg = ProgMsg::new(req.bot(), req.msg(), "Synchronizing...");
            let res = post::handle(req, &mut prog_msg, arg).await;
            prog_msg.map_res(res).await
        }
        Command::Broadcast(arg) => {
            require_admin(req)?;
            let mut prog_msg = ProgMsg::new(req.bot(), req.msg(), "Broadcasting...");
            let res = broadcast::handle(req, &mut prog_msg, arg).await;
            prog_msg.map_res(res).await
        }
    }
}

fn require_private(req: &Request) -> Result<(), Response<'_>> {
    match req.msg().chat.kind {
        ChatKind::Private(_) => Ok(()),
        ChatKind::Public(_) => Err(Response::reply_to(
            "This command is only available in direct messages.",
        )),
    }
}

fn require_admin(req: &Request) -> Result<(), Response<'_>> {
    let admin_tg_user_id = env::var(config::ADMIN_TG_USER_ID_ENV_VAR)
        .ok()
        .and_then(|e| e.parse::<u64>().ok());

    let Some(admin_tg_user_id) = admin_tg_user_id else {
        return Err(Response::reply_to(format!("Admin user id is not set or invalid.\nPlease set env var `{}` on your server.", config::ADMIN_TG_USER_ID_ENV_VAR)))
    };

    if req.msg().from().map(|u| u.id.0) != Some(admin_tg_user_id) {
        Err(Response::reply_to("🎶 Never Gonna Give You Up 🎶"))
    } else {
        Ok(())
    }
}
