use crate::{
    handler::{Request, Response},
    util::text::*,
};

pub async fn handle<'a>(_req: &Request) -> Result<Response<'a>, Response<'a>> {
    Ok(Response::reply_to(
        mtb()
            .bold("Synchronizes Telegram messages to Mastodon.\n\n")
            .plain("Send me /auth in direct message to start.\n\n")
            .italic("(Open sourced on ")
            .link(
                "GitHub",
                "https://github.com/SpriteOvO/tgbot-mastodon-sync"
                    .try_into()
                    .unwrap(),
            )
            .italic(", so you can self-host it)")
            .disable_preview()
            .build(),
    ))
}
