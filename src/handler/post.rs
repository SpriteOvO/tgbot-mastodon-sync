use std::{borrow::Cow, sync::Arc};

use const_format::formatcp;
use spdlog::prelude::*;
use teloxide::{
    net::Download,
    requests::Requester,
    types::{FileMeta, ForwardedFrom, MediaKind::*, Message, MessageEntityKind, User},
};
use tokio::io;

use crate::{
    cmd::{define_cmd_args, Args},
    handler::{
        Request,
        Response::{self, *},
    },
    mastodon::{self, *},
    util::{
        self,
        media::{Media, MediaKind},
        text::MessageText,
    },
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

pub async fn handle(req: &Request, arg: impl Into<String>) -> Result<Response<'_>, Response<'_>> {
    let (state, bot, msg) = (&req.meta.state, &req.meta.bot, &req.meta.msg);

    let args = PostArgs::parse(arg.into())
        .map_err(|err| ReplyTo(format!("Failed to parse arguments.\n\n{err}").into()))?;
    if args.help {
        return Ok(ReplyTo(PostArgs::help().into()));
    }

    let user = msg.from().ok_or_else(|| ReplyTo("No user.".into()))?;

    let Some(reply_to_msg) = msg.reply_to_message() else {
        return Ok(ReplyTo(PostArgs::help().into()));
    };

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

    let (text, entities) = if let Some(media) = media.as_ref() {
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

        (media.caption(), media.entities())
    } else {
        (reply_to_msg.text(), reply_to_msg.entities())
    };

    let mut msg_text = MessageText::new(text.unwrap_or(""), entities.unwrap_or(&[]));
    append_source(&mut msg_text, args.src, reply_to_msg, msg.from());
    let (text, is_formatted) = format_text_for_mastodon(&msg_text);

    status.status(text);
    if is_formatted {
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

fn format_text_for_mastodon<'a>(msg_text: &'a MessageText) -> (Cow<'a, str>, bool) {
    if msg_text.entities().is_empty() {
        return (msg_text.text().into(), false);
    }

    let entities = msg_text.parse_entities();
    let mut text = msg_text.text().to_owned();
    let mut is_formatted = false;

    entities.iter().rev().for_each(|entity| {
        if let MessageEntityKind::TextLink { url } = entity.kind() {
            let (start, end) = (entity.start(), entity.end());

            let mut end_iter = text[end..].chars();
            if end_iter.next() != Some(' ') && end_iter.next().is_some() {
                text.insert(end, ' ');
            }

            text.insert_str(end, &format!("]({url})"));

            let mut start_iter = text[..start].chars().rev();
            let last_char = start_iter.next();
            if last_char.is_some() && last_char != Some(' ') {
                text.insert_str(start, " [");
            } else {
                text.insert(start, '[');
            }
            is_formatted = true;
        }
    });

    (text.into(), is_formatted)
}

fn append_source(
    msg_text: &mut MessageText,
    enable: Option<bool>,
    msg: &Message,
    trigger: Option<&User>,
) {
    const SRC_PREFIX: &str = "\n\n-----\nForward from Telegram";
    const SRC_PREFIX_USER: &str = formatcp!("{SRC_PREFIX} user");

    fn forward_source(msg_text: &mut MessageText, msg: &Message) -> bool {
        let Some(forward) = msg.forward() else {
            return false
        };

        match &forward.from {
            ForwardedFrom::User(user) => {
                msg_text.append_text(format!("{SRC_PREFIX_USER} \"{}\"", user.full_name()));
                if let Some(username) = &user.username {
                    msg_text.append_text(format!(" (@{username})"));
                }
            }
            ForwardedFrom::Chat(chat) => {
                msg_text.append_text(format!(
                    "{SRC_PREFIX} \"{}\"",
                    util::text::chat_display_name(chat)
                ));
                if let Some(username) = &chat.username() {
                    msg_text.append_text(format!(" (@{username})"));
                }
            }
            ForwardedFrom::SenderName(name) => {
                msg_text.append_text(format!("{SRC_PREFIX_USER} \"{name}\""));
            }
        }

        true
    }

    fn sender_source(trigger: Option<&User>, msg_text: &mut MessageText, msg: &Message) -> bool {
        let sender = msg.sender_chat();
        let from = msg.from().filter(|user| {
            (trigger.is_none() || trigger.filter(|t| user.id != t.id).is_some()) // alternative: `.is_some_and()`, still unstable
                && !user.is_anonymous()
                && !user.is_channel()
        });

        if sender.is_none() && from.is_none() {
            return false;
        }

        match (sender, from) {
            (None, None) => unreachable!(),
            (Some(chat), _) => {
                msg_text.append_text(format!(
                    "{SRC_PREFIX_USER} \"{}\"",
                    util::text::chat_display_name(chat)
                ));
                if let Some(username) = &chat.username() {
                    msg_text.append_text(format!(" (@{username})"));
                }
            }
            (None, Some(user)) => {
                msg_text.append_text(format!("{SRC_PREFIX_USER} \"{}\"", user.full_name()));
                if let Some(username) = &user.username {
                    msg_text.append_text(format!(" (@{username})"));
                }
            }
        }

        true
    }

    match enable {
        None => {
            // auto
            //
            // if-then `forward.source`
            // else-if `sender != self` then `sender`
            // else-then `ignore`

            if !forward_source(msg_text, msg) {
                _ = sender_source(trigger, msg_text, msg);
            }
        }
        Some(true) => {
            // force enable
            //
            // if-then `forward.source`
            // else-then `sender`

            if !forward_source(msg_text, msg) {
                _ = sender_source(None, msg_text, msg);
            }
        }
        Some(false) => {
            // force disable
            //
            // `ignore`
        }
    }
}

define_cmd_args! {

r#"Usage: reply /post [option]* to a message

Options:
  help   : show this help message
  +/-src : force enable / disable appending message source (default: auto)
           e.g. +src : sync with message source, including your own message
                -src : sync without any source
                *not-specified* (auto) : sync with message source, excluding your own message
"#

    #[derive(PartialEq, Eq, Debug)]
    pub struct PostArgs {
        pub help: bool,
        pub src: Option<bool>,
    }
}

#[allow(clippy::derivable_impls)]
impl Default for PostArgs {
    fn default() -> Self {
        Self {
            help: false,
            src: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Borrow;

    use super::*;

    #[test]
    fn test_format_text_for_mastodon() {
        let mut msg_text = MessageText::new("", vec![]);

        msg_text.append_text_link("link", "https://example.com".try_into().unwrap());
        msg_text.append_text("def\n");

        msg_text.append_text("abc");
        msg_text.append_text_link("link", "https://example.com".try_into().unwrap());
        msg_text.append_text("def\n");

        msg_text.append_text("abc ");
        msg_text.append_text_link("link", "https://example.com".try_into().unwrap());
        msg_text.append_text("def\n");

        msg_text.append_text("abc ");
        msg_text.append_text_link("link", "https://example.com".try_into().unwrap());
        msg_text.append_text(" def\n");

        msg_text.append_text("abc");
        msg_text.append_text_link("link", "https://example.com".try_into().unwrap());
        msg_text.append_text(" def\n");

        msg_text.append_text("abc");
        msg_text.append_text_link("link", "https://example.com".try_into().unwrap());

        let (formatted, is_formatted) = format_text_for_mastodon(&msg_text);
        assert_eq!(
            (formatted.borrow(), is_formatted),
            (
                r#"[link](https://example.com/) def
abc [link](https://example.com/) def
abc [link](https://example.com/) def
abc [link](https://example.com/) def
abc [link](https://example.com/) def
abc [link](https://example.com/)"#,
                true
            )
        );
    }
}
