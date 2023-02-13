use std::{borrow::Cow, sync::Arc};

use lingua::{Language, LanguageDetector, LanguageDetectorBuilder};
use once_cell::sync::Lazy;
use spdlog::prelude::*;
use teloxide::{
    net::Download,
    prelude::*,
    requests::Requester,
    types::{FileMeta, ForwardedFrom, MediaKind::*, Message, MessageEntityKind, User},
};
use tokio::io;

use crate::{
    cmd::{define_cmd_args, Args},
    config,
    handler::{Request, Response},
    mastodon::{self, Language as MLanguage, *},
    util::{
        self,
        media::{Media, MediaKind},
        text::MessageText,
        ProgMsg,
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

pub async fn handle<'a>(
    req: &Request,
    prog_msg: &mut ProgMsg<'a>,
    arg: impl Into<String>,
) -> Result<Response<'a>, Response<'a>> {
    let args = PostArgs::parse(arg.into())
        .map_err(|err| Response::reply_to(format!("Failed to parse arguments.\n\n{err}")))?;
    if args.help {
        return Ok(Response::reply_to(PostArgs::help()));
    }

    let user = req
        .msg()
        .from()
        .ok_or_else(|| Response::reply_to("No user."))?;

    let Some(reply_to_msg) = req.msg().reply_to_message() else {
        return Ok(Response::reply_to(PostArgs::help()));
    };

    let client = mastodon::Client::new(Arc::clone(req.state()));
    let login_user = client.login(user.id).await.map_err(|err| {
        warn!("user '{}' login mastodon failed: {err}", user.id);
        Response::reply_to("Please use /auth to link your mastodon account first.")
    })?;

    info!("user '{}' trying to post on mastodon", user.id);

    let mut status = StatusBuilder::new();

    status.visibility(Visibility::Public);

    let media = Media::query(req.state(), reply_to_msg)
        .await
        .map_err(|err| {
            error!("user '{}' failed to query media: {err}", user.id);
            Response::reply_to(format!("Failed to query media.\n\n{err}"))
        })?;

    let (text, entities) = if let Some(media) = media.as_ref() {
        let files = media
            .iter()
            .map(filter_media)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                error!("user '{}' trying to sync an unsupported media", user.id);
                Response::reply_to("Contains unsupported media.")
            })?;

        let mut attachments = Vec::with_capacity(files.len());

        info!("downloading media for user '{}'", user.id);

        for (i, file) in files.iter().enumerate() {
            prog_msg
                .update(
                    format!("Processing media... ({}/{})", i + 1, files.len()),
                    false,
                )
                .await;

            let file = req.bot().get_file(&file.id).await.map_err(|err| {
                error!("user '{}' failed to get file meta: {err}", user.id);
                Response::reply_to(format!("Failed to get file meta.\n\n{err}"))
            })?;

            let (reader, writer) = io::duplex(1);

            let (download, attach) = tokio::join!(
                async {
                    // Here seems to be a drop issue? If without this line, later async reads will
                    // freeze. I haven't figured out why.
                    let mut reader = reader;

                    req.bot().download_file(&file.path, &mut reader).await
                },
                async { login_user.attach_media(writer, None).await }
            );

            download.map_err(|err| {
                error!("user '{}' failed to download file: {err}", user.id);
                Response::reply_to(format!("Failed to download file.\n\n{err}"))
            })?;

            let attachment = attach.map_err(|err| {
                error!("user '{}' failed to attach media: {err}", user.id);
                Response::reply_to(format!("Failed to attach media.\n\n{err}"))
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

    prog_msg.update("Detecting content language...", true).await;
    let lang = detect_lang(&msg_text);
    if let Some(lang) = lang {
        status.language(lang);
    }

    let with_src = append_source(
        req.bot(),
        &mut msg_text,
        args.src,
        reply_to_msg,
        req.msg().from(),
    )
    .await;
    let (text, is_formatted) = format_text_for_mastodon(&msg_text);

    status.status(text);
    if is_formatted {
        status.content_type("text/markdown");
    }

    let status = status.build().map_err(|err| {
        error!("user '{}' failed to build status: {err}", user.id);
        Response::reply_to(format!("Failed to build status.\n\n{err}"))
    })?;

    prog_msg.update("Posting status...", true).await;

    let posted_url = login_user.post_status(status).await.map_err(|err| {
        error!("user '{}' failed to post status: {err}", user.id);
        Response::reply_to(format!("Failed to post status on mastodon.\n\n{err}"))
    })?;

    info!(
        "tg user '{}' posted a status: {posted_url} ({lang:?})",
        login_user.tg_user_id(),
    );

    let mut info = String::new();
    if let Some(lang) = lang {
        info.push_str(lang.to_639_1().unwrap_or("??"));
        info.push_str(", ")
    }
    info.push_str(if with_src { "w/ src" } else { "w/o src" });

    Ok(Response::reply_to(format!(
        "Synchronized successfully. \n\n({info})\n{posted_url}",
    ))
    .disable_preview())
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

async fn append_source<'a>(
    bot: &Bot,
    msg_text: &mut MessageText<'a>,
    enable: Option<bool>,
    msg: &Message,
    trigger: Option<&User>,
) -> bool {
    const SRC_PREFIX: &str = "\n\n-----\nFrom";

    async fn forward_source<'a>(bot: &Bot, msg_text: &mut MessageText<'a>, msg: &Message) -> bool {
        let Some(forward) = msg.forward() else {
            return false
        };

        if util::is_from_linked_channel(bot, msg)
            .await
            .unwrap_or(false)
        {
            return false;
        }

        match &forward.from {
            ForwardedFrom::User(user) => {
                msg_text.append_text(format!("{SRC_PREFIX} \"{}\"", user.full_name()));
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
                msg_text.append_text(format!("{SRC_PREFIX} \"{name}\""));
            }
        }

        true
    }

    fn sender_source(
        trigger: Option<&User>,
        exclude_channal: bool,
        msg_text: &mut MessageText,
        msg: &Message,
    ) -> bool {
        let sender = msg.sender_chat();
        let from = msg.from().filter(|user| {
            (trigger.is_none() || trigger.filter(|t| user.id != t.id).is_some()) // alternative: `.is_some_and()`, still unstable
                && !user.is_anonymous()
                && !user.is_channel()
        });

        if sender.is_none() && from.is_none()
            || exclude_channal && sender.map(|s| s.is_channel()).unwrap_or(false)
        {
            return false;
        }

        match (sender, from) {
            (None, None) => unreachable!(),
            (Some(chat), _) => {
                msg_text.append_text(format!(
                    "{SRC_PREFIX} \"{}\"",
                    util::text::chat_display_name(chat)
                ));
                if let Some(username) = &chat.username() {
                    msg_text.append_text(format!(" (@{username})"));
                }
            }
            (None, Some(user)) => {
                msg_text.append_text(format!("{SRC_PREFIX} \"{}\"", user.full_name()));
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
            // else-if `sender != self && sender != channel` then `sender`
            // else-then `ignore`

            forward_source(bot, msg_text, msg).await || sender_source(trigger, true, msg_text, msg)
        }
        Some(true) => {
            // force enable
            //
            // if-then `forward.source`
            // else-then `sender`

            forward_source(bot, msg_text, msg).await || sender_source(None, false, msg_text, msg)
        }
        Some(false) => {
            // force disable
            //
            // `ignore`

            false
        }
    }
}

fn detect_lang(msg_text: &MessageText) -> Option<MLanguage> {
    let content = msg_text.extract_semantics();
    if content.trim().is_empty() {
        return None;
    }

    let lang = detect_lang_inner(content).unwrap_or_else(|| {
        warn!("language connot be reliably detected, fallback to English");
        Language::English
    });

    let lang_code = lang.iso_code_639_3().to_string();

    Some(MLanguage::from_639_3(&lang_code).unwrap_or_else(|| {
        error!("failed to convert ISO-639-3 '{lang_code}', fallback to English");
        MLanguage::Eng
    }))
}

fn detect_lang_inner(text: impl Into<String>) -> Option<Language> {
    static DETECTOR: Lazy<LanguageDetector> =
        Lazy::new(|| LanguageDetectorBuilder::from_languages(config::DETECT_LANGUAGES).build());

    DETECTOR.detect_language_of(text)
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

    #[test]
    fn language_detection() {
        use MessageEntityKind::*;

        {
            let mut msg_text = MessageText::new("", vec![]);
            msg_text.append_text("喵呜 ");
            msg_text.append_text_with_entity("#English ", Hashtag);
            msg_text.append_text_with_entity("#Tags ", Hashtag);
            msg_text.append_text_with_entity("#Hello ", Hashtag);
            msg_text.append_text_with_entity("#World ", Hashtag);
            msg_text.append_text_with_entity("#Example", Hashtag);

            // without `.extract_semantics()`, false positive
            assert_eq!(detect_lang_inner(msg_text.text()), Some(Language::English));

            // with `.extract_semantics()`
            let result = detect_lang(&msg_text).unwrap();
            assert_eq!(result, MLanguage::Zho);
            assert_eq!(result.to_string(), "Chinese");
        }
        {
            let mut msg_text = MessageText::new("", vec![]);
            msg_text.append_text("  \t  \n ");
            msg_text.append_text_with_entity("#English ", Hashtag);
            msg_text.append_text_with_entity("#Tags ", Hashtag);
            msg_text.append_text_with_entity("#Hello ", Hashtag);
            msg_text.append_text_with_entity("#World ", Hashtag);
            msg_text.append_text_with_entity("#Example", Hashtag);
            msg_text.append_text("  \t  \n \n\n");

            assert!(detect_lang(&msg_text).is_none());
        }
        {
            let mut msg_text = MessageText::new("", vec![]);
            msg_text.append_text("喵呜w");

            let result = detect_lang(&msg_text).unwrap();
            assert_eq!(result, MLanguage::Zho);
            assert_eq!(result.to_string(), "Chinese");
        }
        {
            let mut msg_text = MessageText::new("", vec![]);
            msg_text.append_text("Meow~");

            let result = detect_lang(&msg_text).unwrap();
            assert_eq!(result, MLanguage::Eng);
            assert_eq!(result.to_string(), "English");
        }
        {
            let mut msg_text = MessageText::new("", vec![]);
            msg_text.append_text("这是一个 test");

            let result = detect_lang(&msg_text).unwrap();
            assert_eq!(result, MLanguage::Zho);
            assert_eq!(result.to_string(), "Chinese");
        }
        {
            let mut msg_text = MessageText::new("", vec![]);
            msg_text.append_text("The word test in Chinese is 测试");

            let result = detect_lang(&msg_text).unwrap();
            assert_eq!(result, MLanguage::Eng);
            assert_eq!(result.to_string(), "English");
        }
        {
            let mut msg_text = MessageText::new("", vec![]);
            msg_text.append_text("こんにちは 😊");

            let result = detect_lang(&msg_text).unwrap();
            assert_eq!(result, MLanguage::Jpn);
            assert_eq!(result.to_string(), "Japanese");
        }
    }
}
