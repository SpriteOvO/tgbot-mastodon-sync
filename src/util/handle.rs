use std::borrow::Cow;

use teloxide::{prelude::*, types::Me};

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
    ReplyTo(Cow<'a, str>),
    NewMsg(Cow<'a, str>),
}

pub struct Response<'a> {
    pub kind: ResponseKind<'a>,
    pub disable_preview: bool,
}

impl<'a> Response<'a> {
    pub fn nothing() -> Self {
        Self {
            kind: ResponseKind::Nothing,
            disable_preview: false,
        }
    }

    pub fn reply_to(text: impl Into<Cow<'a, str>>) -> Self {
        Self {
            kind: ResponseKind::ReplyTo(text.into()),
            disable_preview: false,
        }
    }

    pub fn new_msg(text: impl Into<Cow<'a, str>>) -> Self {
        Self {
            kind: ResponseKind::NewMsg(text.into()),
            disable_preview: false,
        }
    }

    pub fn disable_preview(mut self) -> Self {
        self.disable_preview = true;
        self
    }
}
