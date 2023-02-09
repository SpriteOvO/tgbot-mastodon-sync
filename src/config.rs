use lingua::Language::{self, *};
use tokio::time::Duration;

pub const BOT_TOKEN_ENV_VAR: &str = "TGBOT_MASTODON_SYNC_BOT_TOKEN";
pub const DB_URL_ENV_VAR: &str = "TGBOT_MASTODON_SYNC_DATABASE_URL";

// We can't support all languages, because that would make the detection very
// slow.
//
// If you want your language to be supported, please open an issue or PR.
pub const DETECT_LANGUAGES: &[Language] = &[Chinese, English, Japanese, Korean, Russian, Ukrainian];

// TODO: make this configurable from CLI
pub const WAITING_FOR_SERVER_PROCESS_MEDIA_INTERVAL: Duration = Duration::from_secs(1);
pub const WAITING_FOR_SERVER_PROCESS_MEDIA_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Package {
    pub name: &'static str,
    pub version: &'static str,
}

pub const PACKAGE: Package = Package {
    name: env!("CARGO_PKG_NAME"),
    version: env!("CARGO_PKG_VERSION"),
};
