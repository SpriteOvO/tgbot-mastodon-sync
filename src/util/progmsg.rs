use teloxide::{
    prelude::*,
    types::{Message, MessageId},
};

pub struct ProgMsg<'a> {
    bot: &'a Bot,
    trigger_msg: &'a Message,
    title: String,
    delete_on_drop: bool,
    history: Vec<String>,
    msg_id: Option<MessageId>,
    last_unsaved: Option<String>,
}

impl<'a> ProgMsg<'a> {
    pub fn new(bot: &'a Bot, trigger_msg: &'a Message, title: impl Into<String>) -> Self {
        Self {
            bot,
            trigger_msg,
            title: title.into(),
            delete_on_drop: true,
            history: vec![],
            msg_id: None,
            last_unsaved: None,
        }
    }

    pub fn set_delete_on_drop(&mut self, enable: bool) {
        self.delete_on_drop = enable;
    }

    pub async fn update(&mut self, status: impl Into<String>, save_to_history: bool) {
        let status = status.into();

        if save_to_history {
            if let Some(last_unsaved) = self.last_unsaved.take() {
                self.history.push(last_unsaved);
            }
        }

        let text = self.format(Some(&status));

        match &self.msg_id {
            None => {
                let msg = self
                    .bot
                    .send_message(self.trigger_msg.chat.id, text)
                    .reply_to_message_id(self.trigger_msg.id)
                    .await;
                if let Ok(msg) = msg {
                    self.msg_id = Some(msg.id);
                }
            }
            Some(msg_id) => {
                _ = self
                    .bot
                    .edit_message_text(self.trigger_msg.chat.id, *msg_id, text)
                    .await;
            }
        }

        if save_to_history {
            self.history.push(status);
        } else {
            self.last_unsaved = Some(status);
        }
    }
}

impl<'a> ProgMsg<'a> {
    fn format(&self, current: Option<impl AsRef<str>>) -> String {
        let mut display = String::new();

        display.push_str(&format!("{}\n\n", self.title));
        for item in &self.history {
            display.push_str(&format!("- {} done\n", item));
        }
        if let Some(current) = current {
            display.push_str(&format!("- {}", current.as_ref()));
        }

        display
    }
}

impl<'a> Drop for ProgMsg<'a> {
    fn drop(&mut self) {
        if let Some(msg_id) = self.msg_id {
            let chat_id = self.trigger_msg.chat.id;
            let bot = self.bot.clone();

            if self.delete_on_drop {
                tokio::spawn(async move {
                    _ = bot.delete_message(chat_id, msg_id).await;
                });
            } else {
                let text = self.format(None as Option<&str>);
                tokio::spawn(async move {
                    _ = bot.edit_message_text(chat_id, msg_id, text).await;
                });
            }
        }
    }
}
