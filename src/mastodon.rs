use std::{str::FromStr, sync::Arc};

use anyhow::{anyhow, bail};
use const_format::formatcp;
use mastodon_async::{
    entities::{attachment::Attachment, status::Status},
    prelude::*,
    registration::Registered,
    scopes, Error as MError,
};
pub use mastodon_async::{Language, StatusBuilder, Visibility};
use serde_json as json;
use spdlog::prelude::*;
use teloxide::types::UserId;
use tokio::{
    fs::File,
    io::AsyncRead,
    time::{self, Duration},
};

use crate::{config, InstanceState};

pub struct Client {
    inst_state: Arc<InstanceState>,
}

impl Client {
    pub fn new(inst_state: Arc<InstanceState>) -> Self {
        Self { inst_state }
    }

    pub async fn login(&self, tg_user_id: UserId) -> anyhow::Result<LoginUser> {
        let login_user = self
            .load_login_user(tg_user_id)
            .await
            .map_err(|err| anyhow!("failed to query user login data: {err}"))?;
        Ok(login_user)
    }

    pub async fn authorization_url(&self, domain: impl AsRef<str>) -> anyhow::Result<String> {
        let domain = domain.as_ref();

        let client = self.register_client(domain).await.map_err(|err| {
            error!("failed to register client for domain '{domain}', err: '{err}'");
            anyhow!("Failed to register client for domain '{domain}\n\n{err}")
        })?;

        Ok(client.authorize_url()?)
    }

    pub async fn authorize(
        &self,
        domain: impl AsRef<str>,
        tg_user_id: UserId,
        auth_code: impl AsRef<str>,
    ) -> anyhow::Result<LoginUser> {
        let domain = domain.as_ref();

        let client = self.register_client(domain).await.map_err(|err| {
            error!("failed to query client for domain '{domain}', err: '{err}'");
            anyhow!("Failed to query client for domain '{domain}\n\n{err}")
        })?;

        let login_user = LoginUser {
            inst: client.complete(auth_code.as_ref()).await?,
            tg_user_id,
        };
        self.save_login_user(tg_user_id, &login_user)
            .await
            .map_err(|err| anyhow!("failed to save user login data: {err}"))?;
        Ok(login_user)
    }

    pub async fn revoke(&self, login_user: &LoginUser) -> anyhow::Result<()> {
        self.delete_login_user(login_user.tg_user_id).await
    }
}

impl Client {
    pub async fn register_client(&self, domain: impl AsRef<str>) -> anyhow::Result<Registered> {
        let domain = domain.as_ref();

        let client = match self.query_client(domain).await {
            Ok(client) => client,
            Err(_) => {
                let client = Registration::new(domain)
                    .client_name(config::PACKAGE.name)
                    .website("https://github.com/SpriteOvO/tgbot-mastodon-sync")
                    .scopes(
                        Scopes::write(scopes::Write::Statuses)
                            .and(Scopes::write(scopes::Write::Media)),
                    )
                    .build()
                    .await?;
                self.save_client(&client).await?;
                client
            }
        };

        Ok(client)
    }
}

impl Client {
    async fn save_login_user(
        &self,
        tg_user_id: UserId,
        login_user: &LoginUser,
    ) -> anyhow::Result<()> {
        let (tg_user_id, login_user_data) = (tg_user_id.0 as i64, login_user.serialize());

        sqlx::query!(
            r#"
INSERT OR REPLACE INTO mastodon_login_user ( tg_user_id, mastodon_async_data )
VALUES ( ?1, ?2 )
        "#,
            tg_user_id,
            login_user_data
        )
        .execute(self.inst_state.db.pool())
        .await?;

        Ok(())
    }

    async fn load_login_user(&self, tg_user_id: UserId) -> anyhow::Result<LoginUser> {
        let tg_user_id_num = tg_user_id.0 as i64;

        let record = sqlx::query!(
            r#"
SELECT mastodon_async_data
FROM mastodon_login_user
WHERE tg_user_id = ?1
        "#,
            tg_user_id_num,
        )
        .fetch_one(self.inst_state.db.pool())
        .await?;

        LoginUser::deserialize(record.mastodon_async_data, tg_user_id)
    }

    async fn delete_login_user(&self, tg_user_id: UserId) -> anyhow::Result<()> {
        let tg_user_id_num = tg_user_id.0 as i64;

        _ = sqlx::query!(
            r#"
DELETE FROM mastodon_login_user
WHERE tg_user_id = ?1
        "#,
            tg_user_id_num,
        )
        .execute(self.inst_state.db.pool())
        .await?;

        Ok(())
    }

    async fn save_client(&self, client: &Registered) -> anyhow::Result<()> {
        let (domain, client_id, client_secret, redirect, scopes, force_login) =
            client.clone().into_parts();
        let scopes = scopes.to_string();

        sqlx::query!(
            r#"
INSERT INTO mastodon_client ( domain, client_id, client_secret, redirect, scopes, force_login )
VALUES ( ?1, ?2, ?3, ?4, ?5, ?6 )
        "#,
            domain,
            client_id,
            client_secret,
            redirect,
            scopes,
            force_login
        )
        .execute(self.inst_state.db.pool())
        .await?;

        Ok(())
    }

    async fn query_client(&self, domain: impl AsRef<str>) -> anyhow::Result<Registered> {
        let domain = domain.as_ref();

        let record = sqlx::query!(
            r#"
SELECT client_id, client_secret, redirect, scopes, force_login
FROM mastodon_client
WHERE domain = ?1
        "#,
            domain,
        )
        .fetch_one(self.inst_state.db.pool())
        .await?;

        Ok(Registered::from_parts(
            domain,
            &record.client_id,
            &record.client_secret,
            &record.redirect,
            FromStr::from_str(&record.scopes)?,
            record.force_login != 0,
        ))
    }
}

pub struct LoginUser {
    inst: Mastodon,
    tg_user_id: UserId,
}

impl LoginUser {
    pub fn domain(&self) -> &str {
        &self.inst.data.base
    }

    pub fn tg_user_id(&self) -> UserId {
        self.tg_user_id
    }

    pub async fn attach_media(
        &self,
        mut data: impl AsyncRead + Unpin,
        description: Option<String>,
    ) -> anyhow::Result<Attachment> {
        // TODO: Do not write out files when https://github.com/dscottboggs/mastodon-async/issues/60 is implemented

        let temp_file = tempfile::Builder::new()
            .prefix(formatcp!(".{}.", config::PACKAGE.name))
            .tempfile()?;
        let temp_file = temp_file.path();

        let mut file = File::create(temp_file).await?;

        trace!("downloading to temp file '{}'", temp_file.display());
        tokio::io::copy(&mut data, &mut file).await?;

        trace!("download done, uploading it");
        let attachment = self.inst.media(temp_file, description).await?;

        trace!("upload done");
        Ok(attachment)
    }

    pub async fn post_status(&self, status: NewStatus) -> anyhow::Result<String> {
        let posted = tokio::select! {
            r = self.post_status_retry(status, config::WAITING_FOR_SERVER_PROCESS_MEDIA_INTERVAL) => r,
            _ = time::sleep(config::WAITING_FOR_SERVER_PROCESS_MEDIA_TIMEOUT) => bail!("timeout waiting for server processing media")
        }?;

        let url = posted.url.unwrap_or_else(|| "*invisible*".to_string());
        Ok(url)
    }
}

impl LoginUser {
    // TODO: Hacky for https://github.com/dscottboggs/mastodon-async/issues/61
    pub async fn post_status_retry(
        &self,
        status: NewStatus,
        interval: Duration,
    ) -> anyhow::Result<Status> {
        loop {
            match self.inst.new_status(status.clone()).await {
                Ok(posted) => return Ok(posted),
                Err(MError::Api { status, response }) => {
                    let err_text = "Cannot attach files that have not finished processing. Try again in a moment!";
                    if status.as_u16() == 422 && response.error == err_text {
                        time::sleep(interval).await;
                        continue;
                    } else {
                        return Err(MError::Api { status, response }.into());
                    }
                }
                Err(err) => return Err(err.into()),
            }
        }
    }

    fn serialize(&self) -> String {
        json::to_string(&self.inst.data).unwrap()
    }

    fn deserialize(input: impl AsRef<str>, tg_user_id: UserId) -> anyhow::Result<Self> {
        let data: Data = json::from_str(input.as_ref())?;
        Ok(Self {
            inst: data.into(),
            tg_user_id,
        })
    }
}
