use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec;
use mcrs_minecraft_registry::RegistryLookup;
use serde::{Deserialize, Serialize};

use crate::item::Template;
use crate::item::ctx::{DecodeCtx, EncodeCtx, Raw};
use crate::item::wire::record_ctx_wire;
use crate::text::Text;
use crate::{Decode, Encode};

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

crate::item::wire::ordinal_enum_wire!(AdvancementType);

/// The wire carries only `background`, `show_toast` and `hidden` as flag bits,
/// so `announce_to_chat` reads back as `false`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayInfo {
    pub icon: Template,
    pub title: Text,
    pub description: Text,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<ResourceLocation>,
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

impl EncodeCtx for DisplayInfo {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.title.encode(&mut w)?;
        self.description.encode(&mut w)?;
        self.icon.encode_ctx(ctx, &mut w)?;
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

impl DecodeCtx<'_> for DisplayInfo {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let title = Text::decode(r)?;
        let description = Text::decode(r)?;
        let icon = Template::decode_ctx(ctx, r)?;
        let frame = AdvancementType::decode(r)?;
        let flags = i32::decode(r)?;
        let background = if flags & FLAG_BACKGROUND != 0 {
            Some(ResourceLocation::decode(r)?)
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
#[derive(Clone, Debug, PartialEq)]
pub struct Advancement {
    pub parent: Option<ResourceLocation>,
    pub display: Option<DisplayInfo>,
    pub requirements: Vec<Vec<String>>,
    pub sends_telemetry_event: bool,
}

record_ctx_wire!(Advancement {
    parent,
    display,
    requirements,
    sends_telemetry_event
});

#[derive(Clone, Debug, PartialEq)]
pub struct PositionedAdvancement {
    pub id: ResourceLocation,
    pub advancement: Advancement,
    pub x: f32,
    pub y: f32,
}

record_ctx_wire!(PositionedAdvancement {
    id,
    advancement,
    x,
    y
});

pub type RawAdvancement = Raw<PositionedAdvancement>;

impl RawAdvancement {
    pub fn from_positioned(
        advancement: &PositionedAdvancement,
        ctx: &dyn RegistryLookup,
    ) -> anyhow::Result<RawAdvancement> {
        Raw::from_value(advancement, ctx)
    }
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
