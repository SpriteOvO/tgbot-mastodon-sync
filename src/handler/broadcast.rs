use std::borrow::Cow;

use teloxide::requests::Requester;

use crate::{
    handler::{Request, Response, UserId},
    util::ProgMsg,
};

pub async fn handle<'a>(
    req: &Request,
    prog_msg: &mut ProgMsg<'a>,
    arg: impl Into<String>,
) -> Result<Response<'a>, Response<'a>> {
    let content = arg.into();
    if content.is_empty() {
        return Err(Response::reply_to("Content cannot be empty."));
    }

    let records = sqlx::query!(
        r#"
SELECT tg_user_id
FROM mastodon_login_user
        "#,
    )
    .fetch_all(req.meta.state.db.pool())
    .await
    .map_err(|err| Response::reply_to(err.to_string()))?;

    for record in records {
        let user_id = UserId(record.tg_user_id as u64);

        let status: Cow<'a, str> = match req.meta.bot.send_message(user_id, &content).await {
            Ok(_) => "succeeded".into(),
            Err(err) => format!("failed ({err})").into(),
        };

        prog_msg
            .update(format!("Boardcast to user '{user_id}' {status}."), true)
            .await;
    }

    prog_msg.set_delete_on_drop(false);

    Ok(Response::nothing())
}
