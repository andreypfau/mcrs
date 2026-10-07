use std::sync::LazyLock;

use mcrs_minecraft_item::component::predicate::ItemPredicate;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::unique_map::UniqueMap;

/// The item each slot range must hold, in the order read.
pub type SlotsPredicate = UniqueMap<SlotRange, ItemPredicate>;

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
