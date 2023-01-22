use std::sync::Arc;

use spdlog::prelude::*;
use teloxide::{net::Download, requests::Requester};
use tokio::io;

use crate::{
    handler::{
        Request,
        Response::{self, *},
    },
    mastodon::{self, *},
    util::media::Media,
};

pub async fn handle(req: &Request) -> Result<Response<'_>, Response<'_>> {
    let user = req.msg.from().ok_or_else(|| ReplyTo("No user.".into()))?;

    let reply_to_msg = req.msg.reply_to_message().ok_or_else(|| {
        ReplyTo("You should reply to a message to be synchronized to mastodon.".into())
    })?;

    let client = mastodon::Client::new(Arc::clone(&req.state));
    let login_user = client.login(user.id).await.map_err(|err| {
        warn!("user '{}' login mastodon failed: {err}", user.id);
        ReplyTo("Please use /auth to link your mastodon account first.".into())
    })?;

    info!("user '{}' trying to post on mastodon", user.id);

    let mut status = StatusBuilder::new();

    status
        .status(reply_to_msg.text().unwrap_or(""))
        .visibility(Visibility::Public)
        .language(Language::Eng);

    if let Some(media) = Media::get(&req.state, reply_to_msg) {
        let mut attachments = Vec::with_capacity(media.len());

        for file in media.iter().filter_map(|m| m.file()) {
            let file = req.bot.get_file(&file.id).await.map_err(|err| {
                error!("user '{}' failed to get file meta: {err}", user.id);
                ReplyTo(format!("Failed to get file meta.\n\n{err}").into())
            })?;

            let (reader, writer) = io::duplex(1);

            let (download, attach) = tokio::join!(
                async {
                    // Here seems to be a drop issue? If without this line, later async reads will
                    // freeze. I haven't figured out why.
                    let mut reader = reader;

                    req.bot.download_file(&file.path, &mut reader).await
                },
                login_user.attach_media(writer, None)
            );

            download.map_err(|err| {
                error!("user '{}' failed to download file: {err}", user.id);
                ReplyTo(format!("Failed to download file.\n\n{err}").into())
            })?;
            let attachment = attach.map_err(|err| {
                error!("user '{}' failed to attach media: {err}", user.id);
                ReplyTo(format!("Failed to attach media.\n\n{err}").into())
            })?;

            attachments.push(attachment);
        }

        status.media_ids(attachments.into_iter().map(|a| a.id));
    }

    let status = status.build().map_err(|err| {
        error!("user '{}' failed to build status: {err}", user.id);
        ReplyTo(format!("Failed to build status.\n\n{err}").into())
    })?;

    let posted_url = login_user.post_status(status).await.map_err(|err| {
        error!("user '{}' failed to post status: {err}", user.id);
        ReplyTo(format!("Failed to post status on mastodon.\n\n{err}").into())
    })?;

    Ok(ReplyTo(
        format!("Synchronized successfully.\n\n{posted_url}").into(),
    ))
}
