use std::fmt;
use std::sync::LazyLock;

use mcrs_minecraft_item::component::predicate::ItemPredicate;
use serde::de::{Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The item each slot range must hold, in the order read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SlotsPredicate(pub Vec<(SlotRange, ItemPredicate)>);

impl Serialize for SlotsPredicate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (range, predicate) in &self.0 {
            map.serialize_entry(range.name(), predicate)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for SlotsPredicate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct SlotsVisitor;

        impl<'de> Visitor<'de> for SlotsVisitor {
            type Value = SlotsPredicate;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of slot ranges to item predicates")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<SlotsPredicate, A::Error> {
                let mut entries: Vec<(SlotRange, ItemPredicate)> = Vec::new();
                while let Some(range) = map.next_key::<SlotRange>()? {
                    if entries.iter().any(|(seen, _)| *seen == range) {
                        return Err(A::Error::custom(format_args!(
                            "Duplicate key '{}'",
                            range.name()
                        )));
                    }
                    entries.push((range, map.next_value()?));
                }
                Ok(SlotsPredicate(entries))
            }
        }

        d.deserialize_map(SlotsVisitor)
    }
}

/// One of the named slot ranges of an entity: a single slot such as
/// `weapon.mainhand`, or a group such as `container.*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotRange(usize);

impl SlotRange {
    pub fn read(name: &str) -> Option<Self> {
        RANGES.iter().position(|range| range == name).map(SlotRange)
    }

    pub fn name(self) -> &'static str {
        &RANGES[self.0]
    }
}

impl Serialize for SlotRange {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for SlotRange {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let name = String::deserialize(d)?;
        SlotRange::read(&name)
            .ok_or_else(|| D::Error::custom(format_args!("Unknown slot range: {name}")))
    }
}

static RANGES: LazyLock<Vec<String>> = LazyLock::new(|| {
    let mut names = Vec::new();
    let mut range = |prefix: &str, size: usize| {
        names.extend((0..size).map(|index| format!("{prefix}{index}")));
        names.push(format!("{prefix}*"));
    };
    range("container.", 54);
    range("hotbar.", 9);
    range("inventory.", 27);
    range("enderchest.", 27);
    range("mob.inventory.", 8);
    range("horse.", 15);
    let mut names: Vec<String> = std::iter::once("contents".to_owned())
        .chain(names)
        .collect();
    names.extend(
        [
            "weapon",
            "weapon.mainhand",
            "weapon.offhand",
            "weapon.*",
            "armor.head",
            "armor.chest",
            "armor.legs",
            "armor.feet",
            "armor.body",
            "armor.*",
            "saddle",
            "horse.chest",
            "player.cursor",
        ]
        .map(str::to_owned),
    );
    names.extend((0..4).map(|index| format!("player.crafting.{index}")));
    names.push("player.crafting.*".to_owned());
    names
});
