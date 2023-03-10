use std::{borrow::Cow, collections::HashMap, sync::Arc};

use once_cell::sync::Lazy;
use spdlog::prelude::*;
use teloxide::types::UserId;
use tokio::sync::Mutex;

use crate::{
    handler::{Request, Response},
    mastodon,
    util::text::*,
};

static AUTH_DOMAIN_CACHE: Lazy<Mutex<HashMap<UserId, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub async fn auth(req: &Request, arg: impl Into<String>) -> Result<Response<'_>, Response<'_>> {
    let user = req
        .msg()
        .from()
        .ok_or_else(|| Response::reply_to("No user."))?;

    let client = mastodon::Client::new(Arc::clone(req.state()));

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

        return Err(Response::reply_to(
            mtb()
                .plain(response)
                .plain("\n\nformat: ")
                .code("/auth <domain or auth-code>")
                .build(),
        ));
    }

    info!("user '{}' trying to auth mastodon", user.id);

    let mut auth_domain_cache = AUTH_DOMAIN_CACHE.lock().await;
    let auth_domain = auth_domain_cache.get(&user.id);
    match auth_domain {
        // Treat as domain
        None => {
            let domain = arg;
            let url = client.authorization_url(&domain).await.map_err(|err| {
                error!("failed to obtain authorization url for domain '{domain}', err: '{err}'");
                Response::reply_to(format!(
                    "Failed to obtain authorization url for domain '{domain}'\n\n{err}"
                ))
            })?;

            auth_domain_cache.insert(user.id, domain);

            Ok(Response::reply_to(
                format!("Please click this link to authorize:\n\n{url}\n\nThen send back the auth code with command /auth."),
            ))
        }
        // Treat as auth code
        Some(domain) => {
            let auth_code = arg;
            let res = client.authorize(domain, user.id, &auth_code).await.map_err(|err| {
                error!("failed to authorize for domain '{domain}' with auth code '{auth_code}'. err: '{err}'");
                Response::reply_to(format!("Failed to authorize for domain '{domain}' with auth code '{auth_code}'.\n\n{err}\n\nPlease send /auth <domain> to restart authorization.", ))
            });

            info!(
                "user '{}' authorized successfully. domain '{domain}'",
                user.id
            );

            auth_domain_cache.remove(&user.id);
            res?;
            Ok(Response::reply_to("Authorized successfully."))
        }
    }
}

pub async fn revoke(req: &Request) -> Result<Response<'_>, Response<'_>> {
    let user = req
        .msg()
        .from()
        .ok_or_else(|| Response::reply_to("No user."))?;

    let client = mastodon::Client::new(Arc::clone(req.state()));

    match client.login(user.id).await {
        Err(_) => Err(Response::reply_to(
            "You have not linked your mastodon account yet.\n\nUsing /auth command to link one.",
        )),
        Ok(login_user) => {
            info!("user '{}' trying to revoke mastodon auth", user.id);

            let domain = login_user.domain();

            client.revoke(&login_user).await.map_err(|err| {
                error!("failed to revoke mastodon auth for domain '{domain}'. err: '{err}'");
                Response::reply_to(format!(
                    "Failed to revoke mastodon auth for domain '{domain}'.\n\n{err}"
                ))
            })?;

            info!(
                "user '{}' revoked mastodon auth for domain '{domain}'",
                user.id
            );

            Ok(Response::reply_to("Revoked successfully."))
        }
    }
}
