use std::{borrow::Cow, collections::HashMap, sync::Arc};

use mastodon_async::registration::Registered;
use once_cell::sync::Lazy;
use spdlog::prelude::*;
use teloxide::types::UserId;
use tokio::sync::Mutex;

use crate::{
    handler::{
        Request,
        Response::{self, *},
    },
    mastodon,
};

static AUTH_REG_CACHE: Lazy<Mutex<HashMap<UserId, Registered>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub async fn auth(req: &Request, arg: impl Into<String>) -> Result<Response<'_>, Response<'_>> {
    let (state, msg) = (&req.meta.state, &req.meta.msg);

    let user = msg.from().ok_or_else(|| ReplyTo("No user.".into()))?;

    let client = mastodon::Client::new(Arc::clone(state));

    let arg = arg.into();
    if arg.is_empty() {
        let response: Cow<_> = match client.login(user.id).await {
            Err(_) => "You have not linked your mastodon account yet.".into(),
            Ok(login_user) => format!(
                "You have already linked your mastodon account for domain '{}'.",
                login_user.domain()
            )
            .into(),
        };

        return Err(ReplyTo(
            format!("{response}\n\nformat: /auth <domain or auth-code>").into(),
        ));
    }

    info!("user '{}' trying to auth mastodon", user.id);

    let mut auth_reg_cache = AUTH_REG_CACHE.lock().await;
    let auth_reg = auth_reg_cache.get(&user.id);

    match auth_reg {
        // Treat as domain
        None => {
            let domain = arg;
            let res = client.auth_step_1(&domain).await;

            let reg = res.map_err(|err| {
                error!("failed to create mastodon client for domain '{domain}'. err: '{err}'");
                ReplyTo(format!("Failed to login mastodon for domain '{domain}'.\n\n{err}").into())
            })?;
            let url = reg.authorize_url().unwrap(); // We have made sure it has a value in `struct Client`

            auth_reg_cache.insert(user.id, reg);

            Ok(ReplyTo(
                format!("Please click this link to authorize:\n\n{url}\n\nThen send back the auth code with command /auth.").into(),
            ))
        }
        // Treat as auth code
        Some(reg) => {
            let auth_code = arg;

            // FIXME: There should be a public method to get the domain, but it's not.
            let domain = reg.clone().into_parts().0;

            let res = client.auth_step_2(reg, user.id, &auth_code).await;
            auth_reg_cache.remove(&user.id);

            let _login_user = res.map_err(|err| {
                error!("failed to authorize for domain '{domain}' with auth code '{auth_code}'. err: '{err}'");
                ReplyTo(format!("Failed to authorize for domain '{domain}' with auth code '{auth_code}'.\n\n{err}\n\nPlease send /auth <domain> to restart authorization.", ).into())
            })?;

            info!(
                "user '{}' authorized successfully. domain '{domain}'",
                user.id
            );

            Ok(ReplyTo("Authorized successfully.".into()))
        }
    }
}

pub async fn revoke(req: &Request) -> Result<Response<'_>, Response<'_>> {
    let (state, msg) = (&req.meta.state, &req.meta.msg);

    let user = msg.from().ok_or_else(|| ReplyTo("No user.".into()))?;

    let client = mastodon::Client::new(Arc::clone(state));

    match client.login(user.id).await {
        Err(_) => Err(ReplyTo(
            "You have not linked your mastodon account yet.\n\nUsing /auth command to link one."
                .into(),
        )),
        Ok(login_user) => {
            info!("user '{}' trying to revoke mastodon auth", user.id);

            let domain = login_user.domain();

            client.revoke(&login_user).await.map_err(|err| {
                error!("failed to revoke mastodon auth for domain '{domain}'. err: '{err}'");
                ReplyTo(
                    format!("Failed to revoke mastodon auth for domain '{domain}'.\n\n{err}")
                        .into(),
                )
            })?;

            info!(
                "user '{}' revoked mastodon auth for domain '{domain}'",
                user.id
            );

            Ok(ReplyTo("Revoked successfully.".into()))
        }
    }
}
