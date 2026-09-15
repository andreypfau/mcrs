use std::sync::Arc;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_core::{HolderSet, ResourceLocation};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use serde::{Deserialize, Serialize};

/// One `spawn_conditions` entry of a variant asset: a priority, and a
/// condition that an absent field leaves always true.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnSelector {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<SpawnCondition>,
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum SpawnCondition {
    #[serde(rename = "minecraft:structure")]
    Structure { structures: HolderSet },
    #[serde(rename = "minecraft:biome")]
    Biome { biomes: HolderSet },
    #[serde(rename = "minecraft:moon_brightness")]
    MoonBrightness { range: DoubleBounds },
}

/// `MinMaxBounds.Doubles`: a bare number is a point, an object holds either
/// bound or both, and a point writes back as the bare number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DoubleBounds {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl DoubleBounds {
    pub fn matches(&self, value: f64) -> bool {
        !self.min.is_some_and(|min| min > value) && !self.max.is_some_and(|max| max < value)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FullBounds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max: Option<f64>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BoundsRepr {
    Point(f64),
    Full(FullBounds),
}

impl<'de> Deserialize<'de> for DoubleBounds {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (min, max) = match BoundsRepr::deserialize(d)? {
            BoundsRepr::Point(value) => (Some(value), Some(value)),
            BoundsRepr::Full(full) => (full.min, full.max),
        };
        if let (Some(min), Some(max)) = (min, max)
            && min > max
        {
            return Err(serde::de::Error::custom(format!(
                "min {min} is above max {max}"
            )));
        }
        Ok(DoubleBounds { min, max })
    }
}

impl Serialize for DoubleBounds {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match (self.min, self.max) {
            (Some(min), Some(max)) if min == max => s.serialize_f64(min),
            (min, max) => FullBounds { min, max }.serialize(s),
        }
    }
}

/// The ids a resolved holder set names, over a registry's index.
pub type IdSet = Arc<FixedBitSet>;

/// What a variant condition is tested against: the structure whose piece
/// holds the spawn, the biome there, and the moon the region sees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpawnContext {
    pub structure: Option<u32>,
    pub biome: u32,
    pub moon_brightness: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    Structure(IdSet),
    Biome(IdSet),
    MoonBrightness(DoubleBounds),
}

impl Condition {
    fn test(&self, ctx: &SpawnContext) -> bool {
        match self {
            Condition::Structure(set) => ctx.structure.is_some_and(|id| set.contains(id as usize)),
            Condition::Biome(set) => set.contains(ctx.biome as usize),
            Condition::MoonBrightness(range) => range.matches(ctx.moon_brightness),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Selector {
    variant: usize,
    priority: i32,
    condition: Option<Condition>,
}

/// A variant registry's spawn selectors, resolved and sorted the way
/// `PriorityProvider.select` walks them: highest priority first, ties in
/// registry then file order.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct VariantTable {
    pub ids: Vec<ResourceLocation>,
    selectors: Vec<Selector>,
}

impl VariantTable {
    /// `entries` in registry order; `structures` and `biomes` resolve a holder
    /// set into the id set a condition tests.
    pub fn freeze<'a>(
        entries: impl IntoIterator<Item = (ResourceLocation, &'a [SpawnSelector])>,
        structures: &dyn Fn(&HolderSet) -> Result<IdSet, String>,
        biomes: &dyn Fn(&HolderSet) -> Result<IdSet, String>,
    ) -> Result<Self, String> {
        let mut table = VariantTable::default();
        for (id, selectors) in entries {
            let variant = table.ids.len();
            for selector in selectors {
                let condition = match &selector.condition {
                    None => None,
                    Some(SpawnCondition::Structure { structures: set }) => Some(
                        Condition::Structure(structures(set).map_err(|e| format!("{id}: {e}"))?),
                    ),
                    Some(SpawnCondition::Biome { biomes: set }) => Some(Condition::Biome(
                        biomes(set).map_err(|e| format!("{id}: {e}"))?,
                    )),
                    Some(SpawnCondition::MoonBrightness { range }) => {
                        Some(Condition::MoonBrightness(*range))
                    }
                };
                table.selectors.push(Selector {
                    variant,
                    priority: selector.priority,
                    condition,
                });
            }
            table.ids.push(id);
        }
        table
            .selectors
            .sort_by_key(|s| std::cmp::Reverse(s.priority));
        Ok(table)
    }

    /// `VariantUtils.selectVariantToSpawn`: the candidates of the highest
    /// priority that passes, one drawn even when it is the only one. `None`
    /// leaves the kind's default variant, as an empty registry would.
    pub fn pick(&self, ctx: &SpawnContext, rng: &mut XoroshiroRandom) -> Option<&ResourceLocation> {
        let mut highest = i32::MIN;
        let mut candidates = Vec::new();
        for selector in &self.selectors {
            if selector.priority < highest {
                break;
            }
            if selector.condition.as_ref().is_none_or(|c| c.test(ctx)) {
                highest = selector.priority;
                candidates.push(selector.variant);
            }
        }
        if candidates.is_empty() {
            return None;
        }
        let index = rng.next_i32_bound(candidates.len() as i32) as usize;
        Some(&self.ids[candidates[index]])
    }
}
/// The variant registries the spawned kinds draw from, each in registry
/// order.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct VariantTables {
    pub cats: VariantTable,
    pub cat_sounds: Vec<ResourceLocation>,
    pub chickens: VariantTable,
    pub chicken_sounds: Vec<ResourceLocation>,
    pub zombie_nautiluses: VariantTable,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_core::ResourceLocation;

    #[test]
    fn a_point_and_a_range_both_round_trip() {
        for text in ["0.9", r#"{"min":0.9}"#, r#"{"min":0.1,"max":0.5}"#, "{}"] {
            let bounds: DoubleBounds = serde_json::from_str(text).unwrap();
            assert_eq!(serde_json::to_string(&bounds).unwrap(), text, "{text}");
        }
        assert!(serde_json::from_str::<DoubleBounds>(r#"{"min":2,"max":1}"#).is_err());
        let at_least: DoubleBounds = serde_json::from_str(r#"{"min":0.9}"#).unwrap();
        assert!(at_least.matches(1.0) && at_least.matches(0.9) && !at_least.matches(0.8));
    }

    #[test]
    fn a_selector_reads_its_typed_condition() {
        let text = r##"{"condition":{"type":"minecraft:structure","structures":"#minecraft:cats_spawn_as_black"},"priority":1}"##;
        let selector: SpawnSelector = serde_json::from_str(text).unwrap();
        assert_eq!(
            selector,
            SpawnSelector {
                condition: Some(SpawnCondition::Structure {
                    structures: HolderSet::Tag(ResourceLocation::minecraft("cats_spawn_as_black")),
                }),
                priority: 1,
            }
        );
        assert_eq!(serde_json::to_string(&selector).unwrap(), text);
        let bare: SpawnSelector = serde_json::from_str(r#"{"priority":0}"#).unwrap();
        assert_eq!(bare.condition, None);
    }
}
