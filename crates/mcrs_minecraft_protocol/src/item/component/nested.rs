use std::fmt;
use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_core::codec::{self, int_value};
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ID, LIST_ID, STRING_ID};
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::{Error as _, IgnoredAny, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::component::{CustomName, Damage, ItemReg, MaxStackSize, Unbreakable};
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::harness::Sample;
use crate::item::kind::ItemComponentKind;
use crate::item::patch::ComponentPatch;
use crate::item::stack::Template;
use crate::text::Text;
use crate::{Bounded, Encode};

macro_rules! delegating_ctx {
    ($($ty:ident),* $(,)?) => {$(
        impl EncodeCtx for $ty {
            fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
                self.0.encode_ctx(ctx, w)
            }
        }

        impl<'a> DecodeCtx<'a> for $ty {
            fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
                DecodeCtx::decode_ctx(ctx, r).map($ty)
            }
        }
    )*};
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UseRemainder(pub Template);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SulfurCubeContent(pub Template);

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BundleContents(pub Vec<Template>);

delegating_ctx!(UseRemainder, SulfurCubeContent, BundleContents);

pub const MAX_CHARGED_PROJECTILES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Default, Serialize)]
#[serde(transparent)]
pub struct ChargedProjectiles {
    items: Bounded<Vec<Template>, MAX_CHARGED_PROJECTILES>,
}

impl ChargedProjectiles {
    pub fn new(items: Vec<Template>) -> anyhow::Result<Self> {
        ensure!(
            items.len() <= MAX_CHARGED_PROJECTILES,
            "Got {} items, but maximum is {MAX_CHARGED_PROJECTILES}",
            items.len()
        );
        Ok(Self {
            items: Bounded(items),
        })
    }

    pub fn items(&self) -> &Vec<Template> {
        &self.items.0
    }
}

impl<'de> Deserialize<'de> for ChargedProjectiles {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let items = Vec::<Template>::deserialize(d)?;
        if items.len() > MAX_CHARGED_PROJECTILES {
            return Err(D::Error::custom(format_args!(
                "List is too long: {}, expected range [0-{MAX_CHARGED_PROJECTILES}]",
                items.len()
            )));
        }
        Ok(Self {
            items: Bounded(items),
        })
    }
}

impl EncodeCtx for ChargedProjectiles {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.items.encode_ctx(ctx, w)
    }
}

impl<'a> DecodeCtx<'a> for ChargedProjectiles {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Self {
            items: Bounded::decode_ctx(ctx, r)?,
        })
    }
}

/// Sides are boxed so a rare four-template value does not size every
/// `ItemComponentValue`.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PotDecorations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub back: Option<Box<Template>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<Box<Template>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<Box<Template>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front: Option<Box<Template>>,
}

impl PotDecorations {
    pub fn sides(&self) -> [Option<&Template>; 4] {
        [&self.back, &self.left, &self.right, &self.front].map(|side| side.as_deref())
    }
}

impl EncodeCtx for PotDecorations {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        for side in self.sides() {
            match side {
                Some(template) => {
                    true.encode(&mut w)?;
                    template.encode_ctx(ctx, &mut w)?;
                }
                None => false.encode(&mut w)?,
            }
        }
        Ok(())
    }
}

impl<'a> DecodeCtx<'a> for PotDecorations {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let mut side = || Option::<Template>::decode_ctx(ctx, r).map(|t| t.map(Box::new));
        Ok(PotDecorations {
            back: side()?,
            left: side()?,
            right: side()?,
            front: side()?,
        })
    }
}

pub const MAX_CONTAINER_SLOTS: usize = 256;

/// `ItemContainerContents`: dense by slot index; the persistent form lists
/// only the occupied slots, so trailing empty slots do not survive a save.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Container {
    slots: Bounded<Vec<Option<Template>>, MAX_CONTAINER_SLOTS>,
}

impl Container {
    pub fn new(slots: Vec<Option<Template>>) -> anyhow::Result<Self> {
        ensure!(
            slots.len() <= MAX_CONTAINER_SLOTS,
            "Got {} items, but maximum is {MAX_CONTAINER_SLOTS}",
            slots.len()
        );
        Ok(Self {
            slots: Bounded(slots),
        })
    }

    pub fn slots(&self) -> &[Option<Template>] {
        &self.slots.0
    }
}

#[derive(Serialize)]
struct SlotEntryRef<'a> {
    slot: i32,
    item: &'a Template,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SlotEntry {
    #[serde(deserialize_with = "int_value")]
    slot: i32,
    item: Template,
}

impl Serialize for Container {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let occupied: Vec<SlotEntryRef> = self
            .slots()
            .iter()
            .enumerate()
            .filter_map(|(slot, item)| {
                item.as_ref().map(|item| SlotEntryRef {
                    slot: slot as i32,
                    item,
                })
            })
            .collect();
        let mut seq = s.serialize_seq(Some(occupied.len()))?;
        for entry in &occupied {
            seq.serialize_element(entry)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Container {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct SlotsVisitor;

        impl<'de> Visitor<'de> for SlotsVisitor {
            type Value = Container;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a list of {slot, item} entries")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Container, A::Error> {
                let mut entries: Vec<SlotEntry> = Vec::new();
                while let Some(entry) = seq.next_element()? {
                    entries.push(entry);
                    if entries.len() == MAX_CONTAINER_SLOTS {
                        let mut total = entries.len();
                        while seq.next_element::<IgnoredAny>()?.is_some() {
                            total += 1;
                        }
                        if total > MAX_CONTAINER_SLOTS {
                            return Err(A::Error::custom(format_args!(
                                "List is too long: {total}, expected range [0-{MAX_CONTAINER_SLOTS}]"
                            )));
                        }
                    }
                }
                let mut slots = Vec::new();
                for entry in entries {
                    let slot = usize::try_from(entry.slot)
                        .ok()
                        .filter(|slot| *slot < MAX_CONTAINER_SLOTS)
                        .ok_or_else(|| {
                            A::Error::custom(format_args!(
                                "Value {} outside of range [0:{}]",
                                entry.slot,
                                MAX_CONTAINER_SLOTS - 1
                            ))
                        })?;
                    if slots.len() <= slot {
                        slots.resize(slot + 1, None);
                    }
                    slots[slot] = Some(entry.item);
                }
                Container::new(slots).map_err(A::Error::custom)
            }
        }

        d.deserialize_seq(SlotsVisitor)
    }
}

impl EncodeCtx for Container {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.slots.encode_ctx(ctx, w)
    }
}

impl<'a> DecodeCtx<'a> for Container {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Self {
            slots: Bounded::decode_ctx(ctx, r)?,
        })
    }
}

fn item(path: &str) -> ResourceKey<ItemReg> {
    ResourceKey::from_location(ResourceLocation::minecraft(path))
}

fn plain(path: &str, count: i32) -> Template {
    Template::new(item(path), count, ComponentPatch::EMPTY).unwrap()
}

fn patched_sword() -> Template {
    let mut patch = ComponentPatch::EMPTY;
    patch.set(MaxStackSize(codec::Bounded(16)));
    patch.set(Damage(codec::Bounded(7)));
    patch.set(CustomName(Text::text("named")));
    patch.set(Unbreakable);
    patch.remove(ItemComponentKind::RepairCost);
    Template::new(item("diamond_sword"), 3, patch).unwrap()
}

fn template_tags(template: &Template) -> Vec<(&'static str, u8)> {
    let mut tags = vec![("", COMPOUND_ID), ("id", STRING_ID)];
    if template.0.count.0 != 1 {
        tags.push(("count", INT_ID));
    }
    if !template.0.components.is_empty() {
        tags.push(("components", COMPOUND_ID));
    }
    tags
}

fn templates() -> Vec<Template> {
    vec![plain("stone", 1), plain("apple", 99), patched_sword()]
}

impl Sample for UseRemainder {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        template_tags(&self.0)
    }

    fn samples() -> Vec<Self> {
        templates().into_iter().map(UseRemainder).collect()
    }
}

impl Sample for SulfurCubeContent {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        template_tags(&self.0)
    }

    fn samples() -> Vec<Self> {
        templates().into_iter().map(SulfurCubeContent).collect()
    }
}

impl Sample for ChargedProjectiles {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", LIST_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            ChargedProjectiles::default(),
            ChargedProjectiles::new(vec![plain("apple", 1)]).unwrap(),
            ChargedProjectiles::new(templates()).unwrap(),
        ]
    }
}

impl Sample for BundleContents {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", LIST_ID)]
    }

    fn samples() -> Vec<Self> {
        let mut inner = ComponentPatch::EMPTY;
        inner.set(BundleContents(vec![plain("stone", 1)]));
        let nested = Template::new(item("bundle"), 1, inner).unwrap();
        vec![
            BundleContents::default(),
            BundleContents(vec![plain("stone", 64), nested, plain("apple", 99)]),
            BundleContents(templates()),
        ]
    }
}

impl Sample for PotDecorations {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        for (name, side) in [
            ("back", &self.back),
            ("left", &self.left),
            ("right", &self.right),
            ("front", &self.front),
        ] {
            if side.is_some() {
                tags.push((name, COMPOUND_ID));
            }
        }
        if self.front.as_deref().is_some_and(|t| t.0.count.0 != 1) {
            tags.push(("front.count", INT_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        vec![
            PotDecorations::default(),
            PotDecorations {
                back: Some(Box::new(plain("stone", 1))),
                right: Some(Box::new(patched_sword())),
                ..Default::default()
            },
            PotDecorations {
                back: Some(Box::new(plain("stone", 1))),
                left: Some(Box::new(plain("stone", 1))),
                right: Some(Box::new(plain("stone", 1))),
                front: Some(Box::new(plain("apple", 2))),
            },
        ]
    }
}

impl Sample for Container {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", LIST_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            Container::default(),
            Container::new(vec![Some(plain("stone", 1))]).unwrap(),
            Container::new(vec![
                Some(plain("stone", 64)),
                None,
                None,
                Some(patched_sword()),
            ])
            .unwrap(),
        ]
    }
}
