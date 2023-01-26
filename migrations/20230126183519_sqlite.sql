CREATE TABLE IF NOT EXISTS "mastodon_client" (
    "domain"        TEXT    NOT NULL UNIQUE,
    "client_id"     TEXT    NOT NULL,
    "client_secret" TEXT    NOT NULL,
    "redirect"      TEXT    NOT NULL,
    "scopes"        TEXT    NOT NULL,
    "force_login"   INTEGER NOT NULL
);
