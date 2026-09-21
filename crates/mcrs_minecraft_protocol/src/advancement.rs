use std::io::Write;

use anyhow::ensure;
use bytes::Bytes;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::default_true;
use mcrs_minecraft_registry::RegistryLookup;
use serde::{Deserialize, Serialize};

use crate::item::Template;
use crate::item::component::common::ordinal_enum;
use crate::item::ctx::{DecodeCtx, EncodeCtx, Opaque};
use crate::text::Text;
use crate::{Decode, Encode};

ordinal_enum! {
    AdvancementType { Task, Challenge, Goal }
}

crate::item::wire::ordinal_enum_wire!(AdvancementType);

fn task() -> AdvancementType {
    AdvancementType::Task
}

fn is_task(frame: &AdvancementType) -> bool {
    *frame == AdvancementType::Task
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_false(value: &bool) -> bool {
    !*value
}

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
    #[serde(default = "task", skip_serializing_if = "is_task")]
    pub frame: AdvancementType,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_toast: bool,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub announce_to_chat: bool,
    #[serde(default, skip_serializing_if = "is_false")]
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

impl EncodeCtx for Advancement {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.parent.encode(&mut w)?;
        self.display.encode_ctx(ctx, &mut w)?;
        self.requirements.encode(&mut w)?;
        self.sends_telemetry_event.encode(w)
    }
}

impl DecodeCtx<'_> for Advancement {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Advancement {
            parent: Option::decode(r)?,
            display: Option::decode_ctx(ctx, r)?,
            requirements: Vec::decode(r)?,
            sends_telemetry_event: bool::decode(r)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdvancementHolder {
    pub id: ResourceLocation,
    pub value: Advancement,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PositionedAdvancement {
    pub holder: AdvancementHolder,
    pub x: f32,
    pub y: f32,
}

impl EncodeCtx for PositionedAdvancement {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.holder.id.encode(&mut w)?;
        self.holder.value.encode_ctx(ctx, &mut w)?;
        self.x.encode(&mut w)?;
        self.y.encode(w)
    }
}

impl DecodeCtx<'_> for PositionedAdvancement {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(PositionedAdvancement {
            holder: AdvancementHolder {
                id: ResourceLocation::decode(r)?,
                value: Advancement::decode_ctx(ctx, r)?,
            },
            x: f32::decode(r)?,
            y: f32::decode(r)?,
        })
    }
}

/// The exact bytes of one positioned advancement, kept so the packet can
/// carry advancements without the registries the icons need.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawAdvancement(pub Bytes);

impl RawAdvancement {
    pub fn resolve(&self, ctx: &dyn RegistryLookup) -> anyhow::Result<PositionedAdvancement> {
        let mut r = &self.0[..];
        let advancement = PositionedAdvancement::decode_ctx(ctx, &mut r)?;
        ensure!(
            r.is_empty(),
            "{} trailing bytes after an advancement",
            r.len()
        );
        Ok(advancement)
    }

    pub fn from_positioned(
        advancement: &PositionedAdvancement,
        ctx: &dyn RegistryLookup,
    ) -> anyhow::Result<RawAdvancement> {
        let mut bytes = Vec::new();
        advancement.encode_ctx(ctx, &mut bytes)?;
        Ok(RawAdvancement(bytes.into()))
    }
}

impl Encode for RawAdvancement {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        Ok(w.write_all(&self.0)?)
    }
}

impl Decode<'_> for RawAdvancement {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let start = *r;
        PositionedAdvancement::decode_ctx(&Opaque, r)?;
        Ok(RawAdvancement(Bytes::copy_from_slice(
            &start[..start.len() - r.len()],
        )))
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
