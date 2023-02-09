use std::borrow::Cow;

use teloxide::types::{
    Chat, ChatKind, Message, MessageEntity, MessageEntityKind, MessageEntityRef, MessageId, User,
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

pub struct MessageText<'a> {
    text: Cow<'a, str>,
    entities: Cow<'a, [MessageEntity]>,
}

impl<'a> MessageText<'a> {
    pub fn new(
        text: impl Into<Cow<'a, str>>,
        entities: impl Into<Cow<'a, [MessageEntity]>>,
    ) -> Self {
        Self {
            text: text.into(),
            entities: entities.into(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn entities(&self) -> &[MessageEntity] {
        &self.entities
    }

    pub fn parse_entities(&self) -> Vec<MessageEntityRef> {
        MessageEntityRef::parse(&self.text, &self.entities)
    }

    pub fn append_text(&mut self, text: impl AsRef<str>) {
        self.text.to_mut().push_str(text.as_ref())
    }

    pub fn append_text_with_entity(&mut self, text: impl AsRef<str>, kind: MessageEntityKind) {
        self.entities.to_mut().push(MessageEntity {
            kind,
            offset: self.text.encode_utf16().count(),
            length: text.as_ref().encode_utf16().count(),
        });
        self.append_text(text);
    }

    pub fn append_text_link(&mut self, link_text: impl AsRef<str>, link: reqwest::Url) {
        self.append_text_with_entity(link_text, MessageEntityKind::TextLink { url: link });
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
