use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use mcrs_core::StaticRegistry;
use mcrs_voxel_math::BlockPos;
use mcrs_protocol::BlockStateId;
use mcrs_random::Random;
use mcrs_random::xoroshiro::XoroshiroRandom;
use mcrs_vanilla::block::definition::Blocks;
use mcrs_vanilla::block::definition::schema::IntProvider;
use mcrs_vanilla::enchantment::EnchantmentData;
use tracing::{debug, warn};

use crate::world::item::component::Enchantments;

/// The dimension's own random stream, as Java's `ServerLevel.getRandom()`. One
/// world, one writer: every sub-app carries its own.
#[derive(Resource)]
pub struct DimensionRandom(pub XoroshiroRandom);

impl Default for DimensionRandom {
    fn default() -> Self {
        DimensionRandom(XoroshiroRandom::new(0))
    }
}

/// A block left the world. `drop_experience` is Java's `spawnAfterBreak`
/// argument: false when the break must not pay out, as a piston move or a
/// creative break does.
#[derive(Clone, Copy, Debug, Message)]
pub struct BlockDestroyed {
    pub state: BlockStateId,
    pub pos: BlockPos,
    pub dim: Entity,
    pub tool: Option<Entity>,
    pub drop_experience: bool,
}

/// Experience owed at a position, with the reason already settled. A furnace,
/// a mob death and a thrown bottle write the same message.
#[derive(Clone, Copy, Debug, Message)]
pub struct AwardExperience {
    pub amount: i32,
    pub pos: BlockPos,
    pub dim: Entity,
}

pub struct ExperiencePlugin;

impl Plugin for ExperiencePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BlockDestroyed>();
        app.add_message::<AwardExperience>();
        app.init_resource::<DimensionRandom>();
        app.add_systems(
            Update,
            (award_block_experience, spawn_experience_orbs).chain(),
        );
    }
}

fn sample(drop: IntProvider, random: &mut XoroshiroRandom) -> i32 {
    match drop {
        IntProvider::Constant(value) => value,
        IntProvider::Uniform {
            min_inclusive,
            max_inclusive,
        } => min_inclusive + random.next_i32_bound(max_inclusive - min_inclusive + 1),
    }
}

/// Java's `EnchantmentValueEffect.RemoveBinomial` draw. Vanilla switches to a
/// gaussian approximation above 128 trials; a block's experience never reaches
/// that, so the exact draw stands in for both.
fn remove_binomial(random: &mut XoroshiroRandom, n: f32, p: f32) -> f32 {
    let mut removed = 0;
    for _ in 0..n as i32 {
        if random.next_f32() < p {
            removed += 1;
        }
    }
    n - removed as f32
}

/// Java's `EnchantmentHelper.processBlockExperience`: every enchantment on the
/// tool gets to modify the amount through its `block_experience` effects.
fn process_block_experience(
    amount: i32,
    enchantments: Option<&Enchantments>,
    registry: &StaticRegistry<EnchantmentData>,
    random: &mut XoroshiroRandom,
) -> i32 {
    let Some(enchantments) = enchantments else {
        return amount;
    };
    let mut value = amount as f32;
    for (id, level) in enchantments.iter() {
        let Some(data) = registry.get_by_raw(id as u32) else {
            continue;
        };
        let Some(effects) = data
            .effects
            .as_ref()
            .and_then(|effects| effects.block_experience.as_ref())
        else {
            continue;
        };
        for conditional in effects {
            if conditional.requirements.is_some() {
                debug!(
                    enchantment = id,
                    "block_experience effect states requirements; no loot context to test them against"
                );
                continue;
            }
            let mut binomial = |n: f32, p: f32| remove_binomial(random, n, p);
            value = conditional.effect.process(level as i32, value, &mut binomial);
        }
    }
    value as i32
}

fn award_block_experience(
    mut destroyed: MessageReader<BlockDestroyed>,
    mut award: MessageWriter<AwardExperience>,
    blocks: Res<Blocks>,
    registry: Res<StaticRegistry<EnchantmentData>>,
    tools: Query<&Enchantments>,
    mut random: ResMut<DimensionRandom>,
) {
    for event in destroyed.read() {
        if !event.drop_experience {
            continue;
        }
        let Some(id) = blocks.state(event.state).experience else {
            continue;
        };
        let sampled = sample(blocks.experience_drop(id), &mut random.0);
        let enchantments = event.tool.and_then(|tool| tools.get(tool).ok());
        let amount = process_block_experience(sampled, enchantments, &registry, &mut random.0);
        if amount > 0 {
            award.write(AwardExperience {
                amount,
                pos: event.pos,
                dim: event.dim,
            });
        }
    }
}

fn spawn_experience_orbs(mut award: MessageReader<AwardExperience>) {
    for event in award.read() {
        warn!(
            amount = event.amount,
            pos = ?event.pos,
            "experience owed but the orb entity does not exist yet"
        );
    }
}
