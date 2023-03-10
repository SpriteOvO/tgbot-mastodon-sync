use std::{borrow::Cow, ops::Add};

use teloxide::{
    payloads::SendMessage,
    prelude::*,
    requests::JsonRequest,
    types::{
        Chat, ChatId, ChatKind, Message, MessageEntity, MessageEntityKind, MessageEntityRef,
        MessageId, User,
    },
    Bot,
};

pub fn chat_display_name(chat: &Chat) -> Cow<str> {
    match &chat.kind {
        ChatKind::Public(chat) => chat.title.as_deref().map(Cow::Borrowed),
        ChatKind::Private(chat) => {
            let (first_name, last_name) = (chat.first_name.as_deref(), chat.last_name.as_deref());
            if let (Some(first_name), Some(last_name)) = (first_name, last_name) {
                Some(format!("{first_name} {last_name}").into())
            } else {
                first_name.xor(last_name).map(Cow::Borrowed)
            }
        }
    }
    .unwrap_or_else(|| "Untitled Chat".into())
}

pub fn message_url(chat: &Chat, msg_id: MessageId) -> Option<reqwest::Url> {
    Message::url_of(chat.id, chat.username(), msg_id)
}

pub fn message_public_url(chat: &Chat, msg_id: MessageId) -> Option<reqwest::Url> {
    if chat.username().is_some() {
        message_url(chat, msg_id)
    } else {
        None
    }
}

pub fn user_url(user: &User) -> Option<reqwest::Url> {
    user.tme_url()
}

#[derive(Clone)]
pub struct MessageText<'a> {
    text: Cow<'a, str>,
    entities: Cow<'a, [MessageEntity]>,
    disable_preview: bool,
}

// Shorthand
pub fn mtb<'a>() -> MessageTextBuilder<'a> {
    MessageText::builder()
}

impl<'a> MessageText<'a> {
    pub fn builder<'b>() -> MessageTextBuilder<'b> {
        MessageTextBuilder {
            text: MessageText::new("", vec![]),
        }
    }

    pub fn new(
        text: impl Into<Cow<'a, str>>,
        entities: impl Into<Cow<'a, [MessageEntity]>>,
    ) -> Self {
        Self {
            text: text.into(),
            entities: entities.into(),
            disable_preview: false,
        }
    }

    pub fn executor(self, bot: &'a Bot) -> MessageTextExecutor<'a> {
        MessageTextExecutor { text: self, bot }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn entities(&self) -> &[MessageEntity] {
        &self.entities
    }

    pub fn disable_preview(&self) -> bool {
        self.disable_preview
    }

    pub fn into_entities(self) -> Vec<MessageEntity> {
        self.entities.into()
    }

    pub fn parse_entities(&self) -> Vec<MessageEntityRef> {
        MessageEntityRef::parse(&self.text, &self.entities)
    }

    pub fn append_text(&mut self, text: impl AsRef<str>) {
        self.text.to_mut().push_str(text.as_ref())
    }

    pub fn prepend_text(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();

        self.entities
            .to_mut()
            .iter_mut()
            .for_each(|entity| entity.offset += text.encode_utf16().count());
        self.text.to_mut().insert_str(0, text);
    }

    pub fn append_text_with_entity(&mut self, text: impl AsRef<str>, kind: MessageEntityKind) {
        self.entities.to_mut().push(MessageEntity {
            kind,
            offset: self.text.encode_utf16().count(),
            length: text.as_ref().encode_utf16().count(),
        });
        self.append_text(text);
    }

    pub fn prepend_text_with_entity(&mut self, text: impl AsRef<str>, kind: MessageEntityKind) {
        let text = text.as_ref();

        self.prepend_text(text);
        self.entities.to_mut().insert(
            0,
            MessageEntity {
                kind,
                offset: 0,
                length: text.encode_utf16().count(),
            },
        );
    }

    pub fn append_text_link(&mut self, link_text: impl AsRef<str>, link: reqwest::Url) {
        self.append_text_with_entity(link_text, MessageEntityKind::TextLink { url: link });
    }

    pub fn prepend_text_link(&mut self, link_text: impl AsRef<str>, link: reqwest::Url) {
        self.prepend_text_with_entity(link_text, MessageEntityKind::TextLink { url: link });
    }

    pub fn append_text_link_fallback(
        &mut self,
        link_text: impl AsRef<str>,
        link: Option<reqwest::Url>,
    ) {
        match link {
            Some(link) => self.append_text_link(link_text, link),
            None => {
                self.text.to_mut().push_str(link_text.as_ref());
            }
        }
    }

    pub fn append(&mut self, other: impl Into<Self>) {
        let mut other = other.into();

        let old_text_len = self.text.encode_utf16().count();

        other
            .entities
            .to_mut()
            .iter_mut()
            .for_each(|entity| entity.offset += old_text_len);

        self.entities.to_mut().append(&mut other.entities.into());
        self.text.to_mut().push_str(&other.text);
    }

    pub fn prepend(&mut self, other: impl Into<Self>) {
        let mut other = other.into();

        self.prepend_text(other.text);
        other.entities.to_mut().append(self.entities.to_mut());
        self.entities = other.entities;
    }

    pub fn extract_semantics(&self) -> String {
        use MessageEntityKind::*;

        self.parse_entities()
            .into_iter()
            .rev()
            .filter(|entity| match entity.kind() {
                Mention
                | Hashtag
                | Cashtag
                | BotCommand
                | Url
                | Email
                | PhoneNumber
                | Pre { .. }
                | CustomEmoji { .. } => true,
                Bold
                | Italic
                | Underline
                | Strikethrough
                | Spoiler
                | Code
                | TextLink { .. }
                | TextMention { .. } => false,
            })
            .fold(self.text().to_owned(), |mut text, entity| {
                text.replace_range(entity.start()..entity.end(), "");
                text
            })
    }
}

impl<'a> From<&'a str> for MessageText<'a> {
    fn from(value: &'a str) -> Self {
        Self::new(value, vec![])
    }
}

impl<'a> From<String> for MessageText<'a> {
    fn from(value: String) -> Self {
        Self::new(value, vec![])
    }
}

impl<'a> Add for MessageText<'a> {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        self.append(rhs);
        self
    }
}

pub struct MessageTextBuilder<'a> {
    text: MessageText<'a>,
}

macro_rules! define_entity_methods {
    ( $( $name:ident => $kind:ident ),+ $(,)? ) => {
        $(pub fn $name(mut self, text: impl AsRef<str>) -> Self {
            self.text
                .append_text_with_entity(text, MessageEntityKind::$kind);
            self
        })+
    };
}

impl<'a> MessageTextBuilder<'a> {
    pub fn plain(mut self, text: impl AsRef<str>) -> Self {
        self.text.append_text(text);
        self
    }

    pub fn link(mut self, text: impl AsRef<str>, url: reqwest::Url) -> Self {
        self.text.append_text_link(text, url);
        self
    }

    define_entity_methods! {
        bold => Bold,
        italic => Italic,
        underline => Underline,
        strikethrough => Strikethrough,
        spoiler => Spoiler,
        code => Code,
    }

    pub fn pre(mut self, text: impl AsRef<str>) -> Self {
        self.text
            .append_text_with_entity(text, MessageEntityKind::Pre { language: None });
        self
    }

    pub fn disable_preview(mut self) -> Self {
        self.text.disable_preview = true;
        self
    }

    pub fn build(self) -> MessageText<'a> {
        self.text
    }
}

pub struct MessageTextExecutor<'a> {
    text: MessageText<'a>,
    bot: &'a Bot,
}

impl<'a> MessageTextExecutor<'a> {
    pub fn send_message(self, chat_id: ChatId) -> JsonRequest<SendMessage> {
        let entities: Vec<_> = self.text.entities.into();

        self.bot
            .send_message(chat_id, self.text.text)
            .entities(entities)
            .disable_web_page_preview(self.text.disable_preview)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appender_text_link() {
        let text = String::new();
        let entities = Vec::new();
        let mut msg_text = MessageText::new(text, entities);

        msg_text.append_text_link("link", "https://example.com".try_into().unwrap());
        assert_eq!(msg_text.text(), "link");
        assert_eq!(
            msg_text.entities(),
            vec![MessageEntity {
                kind: MessageEntityKind::TextLink {
                    url: reqwest::Url::parse("https://example.com").unwrap()
                },
                offset: 0,
                length: 4,
            }]
        );

        msg_text.append_text("\n\n");
        msg_text.append_text_link("喵呜🐱🥰", "https://http.cat".try_into().unwrap());
        assert_eq!(msg_text.text(), "link\n\n喵呜🐱🥰");
        assert_eq!(
            msg_text.entities(),
            vec![
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://example.com".try_into().unwrap()
                    },
                    offset: 0,
                    length: 4,
                },
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://http.cat".try_into().unwrap()
                    },
                    offset: 6,
                    length: 6,
                }
            ]
        );

        msg_text.append_text("\n");
        msg_text.append_text_link_fallback("Meow", "https:://http.cat/200".try_into().ok());
        assert_eq!(msg_text.text(), "link\n\n喵呜🐱🥰\nMeow");
        assert_eq!(
            msg_text.entities(),
            vec![
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://example.com".try_into().unwrap()
                    },
                    offset: 0,
                    length: 4,
                },
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://http.cat".try_into().unwrap()
                    },
                    offset: 6,
                    length: 6,
                }
            ]
        );

        msg_text.prepend_text("Nya!\n");
        assert_eq!(msg_text.text(), "Nya!\nlink\n\n喵呜🐱🥰\nMeow");
        assert_eq!(
            msg_text.entities(),
            vec![
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://example.com".try_into().unwrap()
                    },
                    offset: 5,
                    length: 4,
                },
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://http.cat".try_into().unwrap()
                    },
                    offset: 11,
                    length: 6,
                }
            ]
        );

        msg_text.prepend_text_link("Teapot!\n", "https://http.cat/418".try_into().unwrap());
        assert_eq!(msg_text.text(), "Teapot!\nNya!\nlink\n\n喵呜🐱🥰\nMeow");
        assert_eq!(
            msg_text.entities(),
            vec![
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://http.cat/418".try_into().unwrap()
                    },
                    offset: 0,
                    length: 8,
                },
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://example.com".try_into().unwrap()
                    },
                    offset: 13,
                    length: 4,
                },
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://http.cat".try_into().unwrap()
                    },
                    offset: 19,
                    length: 6,
                }
            ]
        );

        msg_text.append_text("\n");
        msg_text.append(mtb().bold("Fat Cat").build());
        assert_eq!(
            msg_text.text(),
            "Teapot!\nNya!\nlink\n\n喵呜🐱🥰\nMeow\nFat Cat"
        );
        assert_eq!(
            msg_text.entities(),
            vec![
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://http.cat/418".try_into().unwrap()
                    },
                    offset: 0,
                    length: 8,
                },
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://example.com".try_into().unwrap()
                    },
                    offset: 13,
                    length: 4,
                },
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://http.cat".try_into().unwrap()
                    },
                    offset: 19,
                    length: 6,
                },
                MessageEntity {
                    kind: MessageEntityKind::Bold,
                    offset: 31,
                    length: 7,
                }
            ]
        );

        msg_text.prepend(mtb().italic("I'm a ").build());
        assert_eq!(
            msg_text.text(),
            "I'm a Teapot!\nNya!\nlink\n\n喵呜🐱🥰\nMeow\nFat Cat"
        );
        assert_eq!(
            msg_text.entities(),
            vec![
                MessageEntity {
                    kind: MessageEntityKind::Italic,
                    offset: 0,
                    length: 6,
                },
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://http.cat/418".try_into().unwrap()
                    },
                    offset: 6,
                    length: 8,
                },
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://example.com".try_into().unwrap()
                    },
                    offset: 19,
                    length: 4,
                },
                MessageEntity {
                    kind: MessageEntityKind::TextLink {
                        url: "https://http.cat".try_into().unwrap()
                    },
                    offset: 25,
                    length: 6,
                },
                MessageEntity {
                    kind: MessageEntityKind::Bold,
                    offset: 37,
                    length: 7,
                }
            ]
        );
    }

    #[test]
    fn extract_semantics() {
        let text = String::new();
        let entities = Vec::new();
        let mut msg_text = MessageText::new(text, entities);

        msg_text.append_text("meow 🍓 ");
        msg_text.append_text_link("link", "https://example.com".try_into().unwrap());
        msg_text.append_text(" 🐟 cute ");
        msg_text.append_text_with_entity("https://example.com", MessageEntityKind::Url);
        msg_text.append_text(" 🐱 喵呜");

        assert_eq!(
            msg_text.text(),
            "meow 🍓 link 🐟 cute https://example.com 🐱 喵呜"
        );
        assert_eq!(
            msg_text.extract_semantics(),
            "meow 🍓 link 🐟 cute  🐱 喵呜"
        );
    }
}
