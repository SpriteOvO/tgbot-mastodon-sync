use std::slice;

use anyhow::anyhow;
use serde_json as json;
use spdlog::prelude::*;
use teloxide::{
    prelude::*,
    types::{
        FileMeta,
        MediaKind::{self as InnerMediaKind, *},
        MessageId, MessageKind, PhotoSize,
    },
};

use crate::InstanceState;

pub struct MediaKind(InnerMediaKind);

impl MediaKind {
    pub fn inner(&self) -> &InnerMediaKind {
        &self.0
    }

    pub fn file(&self) -> Option<&FileMeta> {
        let file = match &self.0 {
            Animation(m) => &m.animation.file,
            Audio(m) => &m.audio.file,
            Document(m) => &m.document.file,
            Photo(m) => &Self::choice_best_photo(&m.photo).file,
            Sticker(m) => &m.sticker.file,
            Video(m) => &m.video.file,
            VideoNote(m) => &m.video_note.file,
            Voice(m) => &m.voice.file,
            Contact(_) | Game(_) | Venue(_) | Location(_) | Poll(_) | Text(_) | Migration(_) => {
                return None
            }
        };
        Some(file)
    }

    pub fn caption(&self) -> Option<&str> {
        match &self.0 {
            Animation(m) => m.caption.as_deref(),
            Audio(m) => m.caption.as_deref(),
            Document(m) => m.caption.as_deref(),
            Photo(m) => m.caption.as_deref(),
            Text(m) => Some(&*m.text),
            Video(m) => m.caption.as_deref(),
            Voice(m) => m.caption.as_deref(),
            Contact(_) | Game(_) | Venue(_) | Location(_) | Poll(_) | Sticker(_) | VideoNote(_)
            | Migration(_) => None,
        }
    }

    pub fn choice_best_photo(photos: &[PhotoSize]) -> &PhotoSize {
        photos
            .iter()
            .max_by(|a, b| (a.width * b.height).cmp(&(b.width * b.height)))
            .unwrap()
    }
}

impl MediaKind {
    fn serialize(inner: &InnerMediaKind) -> anyhow::Result<String> {
        Ok(json::to_string(inner)?)
    }

    fn deserialize(data: impl AsRef<str>) -> anyhow::Result<Self> {
        Ok(Self(json::from_str(data.as_ref())?))
    }
}

pub enum Media {
    Single(Box<MediaKind>),
    Group {
        medias: Vec<MediaKind>,
        group_id: String,
    },
}

impl Media {
    pub fn iter(&self) -> impl Iterator<Item = &MediaKind> {
        match self {
            Self::Single(media) => slice::from_ref(&**media).iter(),
            Self::Group { medias, .. } => medias.iter(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Group { medias, .. } => medias.len(),
        }
    }

    pub fn caption(&self) -> Option<&str> {
        match self {
            Self::Single(media) => media.caption(),
            Self::Group { medias, .. } => medias.iter().find_map(|m| m.caption()),
        }
    }

    pub async fn query(state: &InstanceState, msg: &Message) -> anyhow::Result<Option<Self>> {
        let msgc = match &msg.kind {
            MessageKind::Common(common) => common,
            _ => return Ok(None),
        };

        let result = match msg.media_group_id() {
            None => Self::Single(Box::new(MediaKind(msgc.media_kind.clone()))),
            Some(media_group_id) => {
                let medias = query_media_group(state, media_group_id)
                    .await
                    .map_err(|err| anyhow!("failed to query media group: {err}"))?;

                Self::Group {
                    medias,
                    group_id: media_group_id.into(),
                }
            }
        };

        Ok(Some(result))
    }
}

pub async fn on_new_message(inst_state: &InstanceState, msg: &Message) {
    let msgc = match &msg.kind {
        MessageKind::Common(common) => common,
        _ => return,
    };

    if let Some(media_group_id) = msg.media_group_id() {
        _ = insert_media_to_group(inst_state, &msgc.media_kind, msg.id, media_group_id)
            .await
            .map_err(|err| {
                error!(
                    "failed to cache media. chat id '{}', msg id '{}', err: '{}'",
                    msg.chat.id, msg.id, err
                );
            });

        trace!(
            "media cached successfully. chat id '{}', msg id '{}'",
            msg.chat.id,
            msg.id
        );
    }
}

async fn insert_media_to_group(
    inst_state: &InstanceState,
    media: &InnerMediaKind,
    msg_id: MessageId,
    media_group_id: impl AsRef<str>,
) -> anyhow::Result<()> {
    let media_json = MediaKind::serialize(media)?;
    let msg_id = msg_id.0;
    let media_group_id = media_group_id.as_ref();

    sqlx::query!(
        r#"
INSERT OR REPLACE INTO telegram_media_group ( group_id, msg_id, media_json )
VALUES ( ?1, ?2, ?3 )
        "#,
        media_group_id,
        msg_id,
        media_json
    )
    .execute(inst_state.db.pool())
    .await?;

    Ok(())
}

async fn query_media_group(
    inst_state: &InstanceState,
    media_group_id: impl AsRef<str>,
) -> anyhow::Result<Vec<MediaKind>> {
    let media_group_id = media_group_id.as_ref();

    let records = sqlx::query!(
        r#"
SELECT media_json
FROM telegram_media_group
WHERE group_id = ?1
ORDER BY msg_id
        "#,
        media_group_id,
    )
    .fetch_all(inst_state.db.pool())
    .await?;

    records
        .into_iter()
        .map(|r| MediaKind::deserialize(r.media_json))
        .collect()
}
