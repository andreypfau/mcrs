use std::fmt;

use mcrs_minecraft_core::codec::PositiveInt;
use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_item::component::predicate::ItemPredicate;
use mcrs_minecraft_predicate::slots::SlotRange;
use mcrs_minecraft_registry::{Holder, HolderList};
use serde::de::{Error as _, MapAccess, SeqAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The slots a loot entry or command reads items from.
#[derive(Debug, Clone, PartialEq)]
pub enum SlotSource {
    Group(HolderList<SlotSource>),
    Typed(Box<TypedSlotSource>),
}

impl RegistryValue for SlotSource {
    type Registry = Self;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TypedSlotSource {
    #[serde(rename = "minecraft:group", alias = "group")]
    Group(Group),
    #[serde(rename = "minecraft:filtered", alias = "filtered")]
    Filtered(Filtered),
    #[serde(rename = "minecraft:limit_slots", alias = "limit_slots")]
    LimitSlots(LimitSlots),
    #[serde(rename = "minecraft:slot_range", alias = "slot_range")]
    SlotRange(Range),
    #[serde(rename = "minecraft:contents", alias = "contents")]
    Contents(Contents),
    #[serde(rename = "minecraft:empty", alias = "empty")]
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub terms: HolderList<SlotSource>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filtered {
    pub slot_source: Holder<SlotSource>,
    pub item_filter: ItemPredicate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LimitSlots {
    pub slot_source: Holder<SlotSource>,
    pub limit: PositiveInt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Range {
    #[serde(default, skip_serializing_if = "SlotTarget::is_container")]
    pub source: SlotTarget,
    pub slots: SlotRange,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contents {
    pub slot_source: Holder<SlotSource>,
    pub component: ContainerComponent,
}

/// Whose slots a range names: an entity, the block entity, or the container.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotTarget {
    This,
    Attacker,
    DirectAttacker,
    AttackingPlayer,
    TargetEntity,
    InteractingEntity,
    BlockEntity,
    #[default]
    Container,
}

impl SlotTarget {
    fn is_container(&self) -> bool {
        *self == SlotTarget::Container
    }
}

/// An item component that holds items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ContainerComponent {
    #[serde(rename = "minecraft:container")]
    Container,
    #[serde(rename = "minecraft:bundle_contents")]
    BundleContents,
    #[serde(rename = "minecraft:charged_projectiles")]
    ChargedProjectiles,
}

impl<'de> Deserialize<'de> for ContainerComponent {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let id = mcrs_minecraft_core::ResourceLocation::read(&String::deserialize(d)?)
            .map_err(D::Error::custom)?;
        match id.as_str() {
            "minecraft:container" => Ok(ContainerComponent::Container),
            "minecraft:bundle_contents" => Ok(ContainerComponent::BundleContents),
            "minecraft:charged_projectiles" => Ok(ContainerComponent::ChargedProjectiles),
            _ => Err(D::Error::custom("No items in component")),
        }
    }
}

impl Serialize for SlotSource {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            SlotSource::Group(terms) => terms.serialize(s),
            SlotSource::Typed(typed) => typed.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for SlotSource {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct SourceVisitor;

        impl<'de> Visitor<'de> for SourceVisitor {
            type Value = SlotSource;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a slot source or a list of them")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<SlotSource, E> {
                HolderList::deserialize(value::StrDeserializer::new(v)).map(SlotSource::Group)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<SlotSource, A::Error> {
                HolderList::deserialize(value::SeqAccessDeserializer::new(seq))
                    .map(SlotSource::Group)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<SlotSource, A::Error> {
                match TypedSlotSource::deserialize(value::MapAccessDeserializer::new(map))? {
                    TypedSlotSource::Group(group) => Ok(SlotSource::Group(group.terms)),
                    typed => Ok(SlotSource::Typed(Box::new(typed))),
                }
            }
        }

        d.deserialize_any(SourceVisitor)
    }
}
