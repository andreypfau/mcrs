use crate::entity::physics::Transform;
use crate::palette::ChunkBlocks;
use crate::world::dimension::InDimension;
use crate::world::lifecycle::level::SectionLevels;
use crate::world::storage::section::SectionIndex;
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::entity::{ContainsEntity, Entity};
use bevy_ecs::query::With;
use bevy_ecs::system::{Local, Query, Res};
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::LocalPos;
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_registry::BlockStateId;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::hash_map::Entry;
use std::hash::Hash;
pub struct ExplosionPlugin;

impl Plugin for ExplosionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, tick_explode);
    }
}

#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct Explosion;

/// The radius of the [Explosion] to be created by detonating an [Explosive].
/// The detonator entity
#[derive(Component, Debug)]
pub struct Detonator(pub Entity);

impl ContainsEntity for Detonator {
    fn entity(&self) -> Entity {
        self.0
    }
}

#[derive(Component, Default, Debug)]
pub struct ExplosionRadius(pub u16);

#[derive(Event, Debug, Eq, PartialEq)]
pub struct BlockExplodedEvent {
    pub dimension: DimEntity,
    pub chunk: ChunkEntity,
    pub block_pos: BlockPos,
    pub block_state_id: BlockStateId,
    pub detonator: Option<Entity>,
}

impl Hash for BlockExplodedEvent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.dimension.hash(state);
        self.block_pos.hash(state);
    }
}

#[derive(Debug, Copy, Clone)]
struct BlockCacheItem {
    pos: BlockPos,
    block: BlockStateId,
    is_air: bool,
    resistance: f32,
    chunk: Option<Entity>,
    should_explode: Option<bool>,
}

struct BlockCache<'a, 'b> {
    map: &'a mut FxHashMap<BlockPos, BlockCacheItem>,
    chunk_index: &'a SectionIndex,
    chunks: &'a Query<'a, 'a, (Entity, &'b ChunkBlocks)>,
    blocks: &'a BlockDefinitions,
}

impl<'a, 'b> BlockCache<'a, 'b> {
    fn get_explosion_block<I>(&mut self, pos: I) -> &mut BlockCacheItem
    where
        I: Into<BlockPos>,
    {
        let pos = pos.into();
        let BlockCache {
            map,
            chunk_index,
            chunks,
            blocks,
        } = self;
        match map.entry(pos) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let chunk_pos = SectionPos::from(pos);
                let item = (|| {
                    let b = chunk_index.get(chunk_pos)?;
                    let (chunk, palette) = chunks.get(b.entity()).ok()?;
                    let block_state = BlockStateId::from(palette.get(LocalPos::from(pos)));
                    let data = blocks.state(block_state);
                    let resistance = (data.explosion_resistance + 0.3) * 0.3;
                    Some(BlockCacheItem {
                        pos,
                        block: block_state,
                        is_air: data.flags.contains(BlockStateFlags::IS_AIR),
                        resistance,
                        chunk: Some(chunk),
                        should_explode: None,
                    })
                })()
                .unwrap_or(BlockCacheItem {
                    pos,
                    block: BlockStateId(0),
                    is_air: true,
                    resistance: 0.0,
                    chunk: None,
                    should_explode: None,
                });
                v.insert(item)
            }
        }
    }
}

type ExplosionEntity = Entity;
type ChunkEntity = Entity;
type DimEntity = Entity;

fn tick_explode(
    mut explosions: Query<
        (
            ExplosionEntity,
            &Transform,
            &InDimension,
            &ExplosionRadius,
            Option<&Detonator>,
        ),
        With<Explosion>,
    >,
    levels: Query<&SectionLevels>,
    dim_chunks: Query<&SectionIndex>,
    chunks: Query<(ChunkEntity, &ChunkBlocks)>,
    mut queue: Local<Parallel<Vec<(ExplosionEntity, Vec<BlockExplodedEvent>)>>>,
    blocks: Res<Blocks>,
    mut commands: Commands,
    mut writer: MessageWriter<BlockSetRequest>,
) {
    explosions.par_iter_mut().for_each_init(
        || queue.borrow_local_mut(),
        |q, (e, transform, dim, radius, detonator)| {
            let center = transform.translation;
            let dim = dim.entity();
            if !levels
                .get(dim)
                .is_ok_and(|levels| levels.is_entity_ticking(SectionPos::from(center)))
            {
                return;
            }
            let Some(dim_chunks) = dim_chunks.get(dim).ok() else {
                return;
            };

            let mut cache_map = FxHashMap::default();
            let mut cache = BlockCache {
                map: &mut cache_map,
                chunk_index: dim_chunks,
                chunks: &chunks,
                blocks: &blocks,
            };

            let blocks = calc_blocks(
                dim,
                center,
                radius.0 as f32,
                &mut rng(),
                false,
                detonator.map(|d| d.entity()),
                &mut cache,
            );
            q.push((e, blocks));
        },
    );

    let mut event_set = deduplicate_blocks(&mut queue, &mut commands);

    writer.write_batch(event_set.drain().map(|event| {
        let dim = event.dimension;
        let block_pos = event.block_pos;
        commands.trigger(event);
        remove_block(dim, block_pos)
    }));
}

#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "world::tick_explode::calc_blocks", skip_all)
)]
fn calc_blocks<R>(
    dimension: DimEntity,
    center: DVec3,
    radius: f32,
    random: &mut R,
    fire: bool,
    detonator: Option<Entity>,
    cache: &mut BlockCache<'_, '_>,
) -> Vec<BlockExplodedEvent>
where
    R: rand::Rng,
{
    let mut ret = Vec::new();
    for inc in CACHED_RAYS.iter() {
        let mut cached_block = cache.get_explosion_block(center);
        let mut curr = center;

        let r = random.random::<f32>();
        let mut power = radius * (r * 0.6 + 0.7);
        loop {
            let block_pos = BlockPos::from(curr);
            if cached_block.pos != block_pos {
                // TODO: direct buf cache
                cached_block = cache.get_explosion_block(block_pos);
            }
            let Some(chunk) = cached_block.chunk else {
                break;
            };
            power -= cached_block.resistance;
            if power > 0.0 && cached_block.should_explode.is_none() {
                // todo: calc
                let should_explode = true;
                cached_block.should_explode = Some(should_explode);

                if should_explode && (fire || !cached_block.is_air) {
                    ret.push(BlockExplodedEvent {
                        dimension,
                        chunk,
                        block_pos,
                        detonator,
                        block_state_id: cached_block.block,
                    });
                }
            }

            power -= 0.225;
            curr += inc;
            if power <= 0.0 {
                break;
            }
        }
    }

    ret
}

#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "world::tick_explode::deduplicate_blocks", skip_all)
)]
fn deduplicate_blocks(
    queue: &mut Parallel<Vec<(ExplosionEntity, Vec<BlockExplodedEvent>)>>,
    commands: &mut Commands,
) -> FxHashSet<BlockExplodedEvent> {
    let mut event_set = FxHashSet::<BlockExplodedEvent>::default();
    for (explosion, events) in queue.drain() {
        commands.entity(explosion).despawn();
        for event in events {
            event_set.insert(event);
        }
    }
    event_set
}

use crate::block_update::{BlockSetRequest, remove_block};
use bevy_ecs::event::Event;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::Commands;
use bevy_math::DVec3;
use bevy_utils::Parallel;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockStateFlags, Blocks};
use rand::{RngExt, rng};
use std::sync::LazyLock;

const N: i32 = 15;
const SCALE: f64 = 0.3;
const POINTS: usize = 1352;

static CACHED_RAYS: LazyLock<[DVec3; POINTS]> = LazyLock::new(|| {
    let mut out: [DVec3; POINTS] = [DVec3::ZERO; POINTS];
    let mut i = 0usize;

    for x in 0..=N {
        for y in 0..=N {
            for z in 0..=N {
                if x == 0 || x == N || y == 0 || y == N || z == 0 || z == N {
                    let xd = (x as f64 / N as f64) * 2.0 - 1.0;
                    let yd = (y as f64 / N as f64) * 2.0 - 1.0;
                    let zd = (z as f64 / N as f64) * 2.0 - 1.0;

                    let mag = (xd * xd + yd * yd + zd * zd).sqrt();

                    out[i] = DVec3::new((xd / mag) * SCALE, (yd / mag) * SCALE, (zd / mag) * SCALE);
                    i += 1;
                }
            }
        }
    }

    assert_eq!(
        i, POINTS,
        "cached_rays: surface-cell count diverged from POINTS; bump POINTS or fix the loop",
    );

    out
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_update::BlockSetRequest;
    use bevy_app::App;
    use bevy_ecs::message::Messages;
    use bevy_ecs::system::System;

    /// Running `tick_explode` against an empty world (no `Explosion`
    /// entities) drains nothing into
    /// `Messages<BlockSetRequest>` because the event set is empty. Negative-
    /// path smoke test that the system does not panic on an empty world and
    /// the buffer stays clean.
    #[test]
    fn tick_explode_with_empty_world_writes_no_block_set_requests() {
        let mut app = App::new();
        app.add_message::<BlockSetRequest>();
        app.add_plugins(ExplosionPlugin);

        let world = app.world_mut();
        let mut sys = bevy_ecs::system::IntoSystem::into_system(tick_explode);
        sys.initialize(world);
        let _ = sys.run((), world);
        sys.apply_deferred(world);

        let msgs = app.world().resource::<Messages<BlockSetRequest>>();
        assert!(
            msgs.is_empty(),
            "with no explosions in the world, tick_explode must not write any BlockSetRequest \
             (the iterated event set is empty, so no writes are emitted)"
        );
    }
}
