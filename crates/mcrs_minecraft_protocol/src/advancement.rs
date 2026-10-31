use std::io::Write;

use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_core::codec;
use mcrs_minecraft_item::Text;
use mcrs_minecraft_registries::{FromWire, Refs, Resolved, Wire, registry_value};
use serde::{Deserialize, Serialize};

use crate::item::Template;
use crate::{Decode, Encode, Result};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdvancementType {
    #[default]
    Task,
    Challenge,
    Goal,
}

impl AdvancementType {
    pub const ALL: &'static [Self] = &[Self::Task, Self::Challenge, Self::Goal];
}

impl Encode for AdvancementType {
    fn encode(&self, w: impl Write) -> Result<()> {
        crate::VarInt(*self as i32).encode(w)
    }
}

/// An unknown ordinal reads as `Task`.
impl Decode<'_> for AdvancementType {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let id = crate::VarInt::decode(r)?.0;
        Ok(usize::try_from(id)
            .ok()
            .and_then(|id| Self::ALL.get(id))
            .copied()
            .unwrap_or_default())
    }
}

/// The wire carries only `background`, `show_toast` and `hidden` as flag bits,
/// so `announce_to_chat` reads back as `false`.
#[registry_value]
#[derive(FromWire)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayInfo<M: Refs = Resolved> {
    pub icon: Template<M>,
    pub title: Text<M::Nbt>,
    pub description: Text<M::Nbt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<ResourceLocationBuf>,
    #[serde(default, skip_serializing_if = "codec::is_default")]
    pub frame: AdvancementType,
    #[serde(default = "codec::default_true", skip_serializing_if = "Clone::clone")]
    pub show_toast: bool,
    #[serde(default = "codec::default_true", skip_serializing_if = "Clone::clone")]
    pub announce_to_chat: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

const FLAG_BACKGROUND: i32 = 1;
const FLAG_SHOW_TOAST: i32 = 2;
const FLAG_HIDDEN: i32 = 4;

impl Encode for DisplayInfo<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.title.encode(&mut w)?;
        self.description.encode(&mut w)?;
        self.icon.encode(&mut w)?;
        self.frame.encode(&mut w)?;
        let mut flags = 0;
        if self.background.is_some() {
            flags |= FLAG_BACKGROUND;
        }
        if self.show_toast {
            flags |= FLAG_SHOW_TOAST;
        }
        if self.hidden {
            flags |= FLAG_HIDDEN;
        }
        flags.encode(&mut w)?;
        if let Some(background) = &self.background {
            background.encode(&mut w)?;
        }
        Ok(())
    }
}

impl Decode<'_> for DisplayInfo<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let title = Text::decode(r)?;
        let description = Text::decode(r)?;
        let icon = Template::decode(r)?;
        let frame = AdvancementType::decode(r)?;
        let flags = i32::decode(r)?;
        let background = if flags & FLAG_BACKGROUND != 0 {
            Some(ResourceLocationBuf::decode(r)?)
        } else {
            None
        };
        Ok(DisplayInfo {
            icon,
            title,
            description,
            background,
            frame,
            show_toast: flags & FLAG_SHOW_TOAST != 0,
            announce_to_chat: false,
            hidden: flags & FLAG_HIDDEN != 0,
        })
    }
}

/// The part of an advancement a client needs: rewards and criteria stay on
/// the server.
#[derive(Clone, Debug, PartialEq, FromWire, Encode, Decode)]
pub struct Advancement<M: Refs = Resolved> {
    pub parent: Option<ResourceLocationBuf>,
    pub display: Option<DisplayInfo<M>>,
    pub requirements: Vec<Vec<String>>,
    pub sends_telemetry_event: bool,
}

#[derive(Clone, Debug, PartialEq, FromWire, Encode, Decode)]
pub struct PositionedAdvancement<M: Refs = Resolved> {
    pub id: ResourceLocationBuf,
    pub advancement: Advancement<M>,
    pub x: f32,
    pub y: f32,
}

/// When a criterion was obtained, as epoch milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Encode, Decode)]
pub struct CriterionProgress {
    pub obtained: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Default, Encode, Decode)]
pub struct AdvancementProgress {
    pub criteria: Vec<(String, CriterionProgress)>,
}
