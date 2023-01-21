use crate::handler::{Request, Response};

pub async fn handle<'a>(_req: &Request, arg: &'a str) -> Result<Response<'a>, Response<'a>> {
    Err(Response::ReplyTo(arg.into()))
}
