pub const BOT_TOKEN_ENV_VAR: &str = "TGBOT_MASTODON_SYNC_BOT_TOKEN";
pub const DB_URL_ENV_VAR: &str = "TGBOT_MASTODON_SYNC_DATABASE_URL";

pub struct Package {
    pub name: &'static str,
    pub version: &'static str,
}

pub const PACKAGE: Package = Package {
    name: env!("CARGO_PKG_NAME"),
    version: env!("CARGO_PKG_VERSION"),
};
