use crate::handler::{Request, Response};

pub async fn handle<'a>(_req: &Request) -> Result<Response<'a>, Response<'a>> {
    Ok(Response::ReplyTo(
        r#"Synchronizes Telegram messages to Mastodon.

GitHub: https://github.com/SpriteOvO/tgbot-mastodon-sync

Send me /auth in direct message to start."#
            .into(),
    ))
}
