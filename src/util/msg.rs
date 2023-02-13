use anyhow::anyhow;
use teloxide::{prelude::*, types::Message};

// expensive
pub async fn is_from_linked_channel(bot: &Bot, msg: &Message) -> anyhow::Result<bool> {
    let sender_chat = msg.sender_chat().ok_or_else(|| anyhow!("No sender chat"))?;
    let channel_id = bot.get_chat(msg.chat.id).await?.linked_chat_id();

    Ok(channel_id == Some(sender_chat.id.0))
}
