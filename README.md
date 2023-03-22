# tgbot-mastodon-sync

[![](https://img.shields.io/badge/github-tgbot--mastodon--sync-blue?style=flat-square&logo=github)](https://github.com/SpriteOvO/tgbot-mastodon-sync)
[![](https://img.shields.io/crates/v/tgbot-mastodon-sync?style=flat-square&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K)](https://crates.io/crates/tgbot-mastodon-sync)
[![](https://img.shields.io/github/actions/workflow/status/SpriteOvO/tgbot-mastodon-sync/CI.yml?branch=main&style=flat-square&logo=githubactions&logoColor=white)](https://github.com/SpriteOvO/tgbot-mastodon-sync/actions/workflows/CI.yml)

A Telegram bot synchronizes Telegram messages to Mastodon.

Official hosted account: [@mastodon_sync_bot](https://t.me/mastodon_sync_bot)

## Self-Host

### 1. Install binary

You have two ways to install the binary.

- Install from [crates.io](https://crates.io/) registry.

  ```bash
  cargo install tgbot-mastodon-sync
  ```

- Install from git repository.

  ```bash
  git clone https://github.com/SpriteOvO/tgbot-mastodon-sync.git --recursive
  cd tgbot-mastodon-sync
  git checkout <latest-version>
  cargo install --path .
  ```

Both ways are build from source.

### 2. Configure database

This project uses `sqlite` as the database engine.

Choose a file path for the database, its URL will be:

```
sqlite:absolute/path/to/database.sqlite
```

You may want to add `?mode=rwc` to the end of the URL, which will make the bot automatically create the database file if it doesn't exist.

```
sqlite:absolute/path/to/database.sqlite?mode=rwc
```

Tables will be created / migrated automatically by the bot at startup.

### 3. Start the bot

Configure environment variables:

  - `TGBOT_MASTODON_SYNC_BOT_TOKEN`
  - `TGBOT_MASTODON_SYNC_DATABASE_URL`

Run `tgbot-mastodon-sync`.

## Note

- The bot requires [privacy mode](https://core.telegram.org/bots/features#privacy-mode) to be turned off, because media groups need to be cached in advance.

- The database and memory may contain secret data, so pay attention to security.

## License

This project is licensed under [GNU AGPL-3.0 License](/LICENSE).
