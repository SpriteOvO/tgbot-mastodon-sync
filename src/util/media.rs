use std::slice;

use teloxide::{
    prelude::*,
    types::{FileMeta, MediaKind as InnerMediaKind, MessageKind, PhotoSize},
};

use crate::InstanceState;

pub struct MediaKind(InnerMediaKind);

impl MediaKind {
    pub fn inner(&self) -> &InnerMediaKind {
        &self.0
    }

    pub fn file(&self) -> Option<&FileMeta> {
        let file = match &self.0 {
            InnerMediaKind::Animation(m) => &m.animation.file,
            InnerMediaKind::Audio(m) => &m.audio.file,
            InnerMediaKind::Document(m) => &m.document.file,
            InnerMediaKind::Photo(m) => &Self::choice_best_photo(&m.photo).file,
            InnerMediaKind::Sticker(m) => &m.sticker.file,
            InnerMediaKind::Video(m) => &m.video.file,
            InnerMediaKind::VideoNote(m) => &m.video_note.file,
            InnerMediaKind::Voice(m) => &m.voice.file,
            _ => return None,
        };
        Some(file)
    }

    pub fn choice_best_photo(photos: &[PhotoSize]) -> &PhotoSize {
        photos
            .iter()
            .max_by(|a, b| (a.width * b.height).cmp(&(b.width * b.height)))
            .unwrap()
    }
}

pub enum Media {
    Single(Box<MediaKind>),
    Group(Vec<MediaKind>),
}

impl Media {
    pub fn iter(&self) -> impl Iterator<Item = &MediaKind> {
        match self {
            Self::Single(media) => slice::from_ref(&**media).iter(),
            Self::Group(media_vec) => media_vec.iter(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Group(media_vec) => media_vec.len(),
        }
    }

    pub fn get(state: &InstanceState, msg: &Message) -> Option<Self> {
        let msgc = match &msg.kind {
            MessageKind::Common(common) => common,
            _ => return None,
        };

        match msg.media_group_id() {
            None => Some(Self::Single(Box::new(MediaKind(msgc.media_kind.clone())))),
            Some(media_group_id) => {
                unimplemented!()
            }
        }
    }
}
