use crate::handler::{Request, Response};

pub async fn handle<'a>(_req: &Request) -> Result<Response<'a>, Response<'a>> {
    Err(Response::ReplyTo("Pong!".into()))
}
