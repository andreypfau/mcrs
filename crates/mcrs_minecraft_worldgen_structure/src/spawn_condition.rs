use std::sync::Arc;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_item::component::common::MinMaxBounds;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_registry::HolderSet;
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
#[serde(remote = "Self", deny_unknown_fields)]
pub enum SpawnCondition {
    Structure {
        structures: HolderSet<crate::Structure>,
    },
    Biome {
        biomes: HolderSet<mcrs_minecraft_biome::Biome>,
    },
    MoonBrightness {
        range: MinMaxBounds<f64>,
    },
}

mcrs_minecraft_registry::dispatch! {
    SpawnCondition, key = "type", registry = crate::keys::SpawnConditionType,
    {
        Structure => Structure,
        MoonBrightness => MoonBrightness,
        Biome => Biome,
    }
}

/// The ids a resolved holder set names, over a registry's index.
pub type IdSet = Arc<FixedBitSet>;

/// What a variant condition is tested against: the structure whose piece
/// holds the spawn, the biome there, and the moon the region sees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpawnContext {
    pub structure: Option<u16>,
    pub biome: u16,
    pub moon_brightness: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    Structure(IdSet),
    Biome(IdSet),
    MoonBrightness(MinMaxBounds<f64>),
}

impl Condition {
    fn test(&self, ctx: &SpawnContext) -> bool {
        match self {
            Condition::Structure(set) => ctx
                .structure
                .is_some_and(|id| set.contains(usize::from(id))),
            Condition::Biome(set) => set.contains(usize::from(ctx.biome)),
            Condition::MoonBrightness(range) => range.matches(ctx.moon_brightness),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Selector {
    variant: u16,
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
        structures: &dyn Fn(&HolderSet<crate::Structure>) -> Result<IdSet, String>,
        biomes: &dyn Fn(&HolderSet<mcrs_minecraft_biome::Biome>) -> Result<IdSet, String>,
    ) -> Result<Self, String> {
        let mut table = VariantTable::default();
        for (id, selectors) in entries {
            let variant = u16::try_from(table.ids.len())
                .map_err(|_| format!("{id}: the variant registry holds more than 65536 entries"))?;
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
    pub fn pick(&self, ctx: &SpawnContext, rng: &mut WorldgenRandom) -> Option<&ResourceLocation> {
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
        Some(&self.ids[usize::from(candidates[index])])
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
