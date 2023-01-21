use std::sync::Arc;

use spdlog::prelude::*;

use crate::{
    handler::{Request, Response},
    mastodon,
};

pub async fn handle(req: &Request) -> Result<Response<'_>, Response<'_>> {
    let user = req
        .msg
        .from()
        .ok_or_else(|| Response::ReplyTo("No user.".into()))?;

    let reply_to_msg = req.msg.reply_to_message().ok_or_else(|| {
        Response::ReplyTo("You should reply to a message to be synchronized to mastodon.".into())
    })?;

    let client = mastodon::Client::new(Arc::clone(&req.state));
    let login_user = client.login(user.id).await.map_err(|err| {
        warn!("user '{}' login mastodon failed: {err}", user.id);
        Response::ReplyTo("Please use /auth to link your mastodon account first.".into())
    })?;

    info!("user '{}' trying to post on mastodon", user.id);

    let posted_url = login_user
        .post_status(reply_to_msg.text().unwrap_or(""))
        .await
        .map_err(|err| {
            Response::ReplyTo(format!("Failed to post status on mastodon.\n\n{err}").into())
        })?;

    Ok(Response::ReplyTo(
        format!("Synchronized successfully.\n\n{posted_url}").into(),
    ))
}
