use teloxide::{prelude::*, types::Me};

use crate::util::text::*;

pub enum RequestKind<C> {
    NewMessage,
    EditedMessage,
    Command(C),
}

pub struct Request<S, C> {
    state: S,
    bot: Bot,
    me: Me,
    msg: Message,
    kind: RequestKind<C>,
}

impl<S, C> Request<S, C> {
    pub fn new_message(state: S, bot: Bot, me: Me, msg: Message) -> Self {
        Self {
            state,
            bot,
            me,
            msg,
            kind: RequestKind::NewMessage,
        }
    }

    pub fn edited_message(state: S, bot: Bot, me: Me, msg: Message) -> Self {
        Self {
            state,
            bot,
            me,
            msg,
            kind: RequestKind::EditedMessage,
        }
    }

    pub fn new_command(state: S, bot: Bot, me: Me, msg: Message, cmd: C) -> Self {
        Self {
            state,
            bot,
            me,
            msg,
            kind: RequestKind::Command(cmd),
        }
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn bot(&self) -> &Bot {
        &self.bot
    }

    pub fn me(&self) -> &Me {
        &self.me
    }

    pub fn msg(&self) -> &Message {
        &self.msg
    }

    pub fn kind(&self) -> &RequestKind<C> {
        &self.kind
    }
}

pub enum ResponseKind<'a> {
    Nothing,
    ReplyTo(MessageText<'a>),
    NewMsg(MessageText<'a>),
}

pub struct Response<'a> {
    pub kind: ResponseKind<'a>,
}

impl<'a> Response<'a> {
    pub fn nothing() -> Self {
        Self {
            kind: ResponseKind::Nothing,
        }
    }

    pub fn reply_to(text: impl Into<MessageText<'a>>) -> Self {
        Self {
            kind: ResponseKind::ReplyTo(text.into()),
        }
    }

    pub fn new_msg(text: impl Into<MessageText<'a>>) -> Self {
        Self {
            kind: ResponseKind::NewMsg(text.into()),
        }
    }
}
