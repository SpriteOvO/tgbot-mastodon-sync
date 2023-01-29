use std::sync::Arc;

use spdlog::prelude::*;
use teloxide::{
    net::Download,
    requests::Requester,
    types::{FileMeta, MediaKind::*, MessageEntity, MessageEntityKind, MessageEntityRef},
};
use tokio::io;

use crate::{
    handler::{
        Request,
        Response::{self, *},
    },
    mastodon::{self, *},
    util::media::{Media, MediaKind},
};

fn filter_media(media: &MediaKind) -> Option<&FileMeta> {
    let file = media.file()?;

    match media.inner() {
        Animation(_) | Photo(_) | Sticker(_) | Video(_) | VideoNote(_) => true,
        Audio(_) | Contact(_) | Document(_) | Game(_) | Venue(_) | Location(_) | Poll(_)
        | Text(_) | Voice(_) | Migration(_) => false,
    }
    .then_some(file)
}

pub async fn handle(req: &Request) -> Result<Response<'_>, Response<'_>> {
    let (state, bot, msg) = (&req.meta.state, &req.meta.bot, &req.meta.msg);

    let user = msg.from().ok_or_else(|| ReplyTo("No user.".into()))?;

    let reply_to_msg = msg.reply_to_message().ok_or_else(|| {
        ReplyTo("You should reply to a message to be synchronized to mastodon.".into())
    })?;

    let client = mastodon::Client::new(Arc::clone(state));
    let login_user = client.login(user.id).await.map_err(|err| {
        warn!("user '{}' login mastodon failed: {err}", user.id);
        ReplyTo("Please use /auth to link your mastodon account first.".into())
    })?;

    info!("user '{}' trying to post on mastodon", user.id);

    let mut status = StatusBuilder::new();

    status
        .visibility(Visibility::Public)
        .language(Language::Eng);

    let media = Media::query(state, reply_to_msg).await.map_err(|err| {
        error!("user '{}' failed to query media: {err}", user.id);
        ReplyTo(format!("Failed to query media.\n\n{err}").into())
    })?;

    let (text, formatted) = if let Some(media) = media {
        let files = media
            .iter()
            .map(filter_media)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                error!("user '{}' trying to sync an unsupported media", user.id);
                ReplyTo("Contains unsupported media.".into())
            })?;

        let mut attachments = Vec::with_capacity(files.len());

        info!("downloading media for user '{}'", user.id);

        for file in files {
            let file = bot.get_file(&file.id).await.map_err(|err| {
                error!("user '{}' failed to get file meta: {err}", user.id);
                ReplyTo(format!("Failed to get file meta.\n\n{err}").into())
            })?;

            let (reader, writer) = io::duplex(1);

            let (download, attach) = tokio::join!(
                async {
                    // Here seems to be a drop issue? If without this line, later async reads will
                    // freeze. I haven't figured out why.
                    let mut reader = reader;

                    bot.download_file(&file.path, &mut reader).await
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

        status
            .media_ids(attachments.into_iter().map(|a| a.id))
            .sensitive(media.iter().any(|media| media.has_media_spoiler()));

        format_text(media.caption(), media.entities())
    } else {
        format_text(reply_to_msg.text(), reply_to_msg.entities())
    };
    status.status(text);
    if formatted {
        status.content_type("text/markdown");
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

fn format_text<'a>(
    caption: Option<&'a str>,
    entities: Option<&'a [MessageEntity]>,
) -> (String, bool) {
    let caption = caption.unwrap_or("");
    if caption.is_empty() {
        return (String::new(), false);
    }

    if entities.is_none() || entities.unwrap().is_empty() {
        return (String::new(), false);
    }

    let entities = MessageEntityRef::parse(caption, entities.unwrap());
    let mut caption = caption.to_owned();
    let mut formatted = false;

    entities.iter().rev().for_each(|entity| {
        if let MessageEntityKind::TextLink { url } = entity.kind() {
            caption.insert_str(entity.end(), &format!("]({url}) "));
            caption.insert_str(entity.start(), " [");
            formatted = true;
        }
    });

    (caption, formatted)
}
