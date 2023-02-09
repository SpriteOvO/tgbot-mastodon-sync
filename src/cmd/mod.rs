mod arg;

pub(crate) use arg::*;
use teloxide::utils::command::BotCommands;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub enum Command {
    #[command(description = "off")]
    Ping,
    #[cfg(debug_assertions)]
    #[command(description = "off")]
    Debug(String),
    #[command(description = "off")]
    Start,
    #[command(description = "link your mastodon account")]
    Auth(String),
    #[command(description = "unlink your mastodon account")]
    Revoke,
    #[command(
        description = "post the message you replied to mastodon (send with `help` for advanced usages)"
    )]
    Post(String),
    #[command(description = "off")]
    Broadcast(String),
}
