CREATE TABLE IF NOT EXISTS "mastodon_login_user" (
    "tg_user_id"          INTEGER NOT NULL UNIQUE,
    "mastodon_async_data" TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS "telegram_media_group" (
    "group_id"   TEXT    NOT NULL,
    "msg_id"     INTEGER NOT NULL,
    "media_json" TEXT    NOT NULL
);
