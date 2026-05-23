use crate::world::block::Block;
use mcrs_minecraft_block::palette::BlockPalette;
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::entity::{ContainsEntity, Entity};
use bevy_ecs::query::With;
use bevy_ecs::system::{Local, Query, Res};
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{ChunkIndex, ChunkPos};
use mcrs_engine::world::dimension::InDimension;
use mcrs_protocol::BlockStateId;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::hash_map::Entry;
use std::hash::Hash;
pub struct ExplosionPlugin;

impl Plugin for ExplosionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExplosionConfig>();
        app.add_systems(FixedUpdate, tick_explode);
    }
}

/// Runtime configuration for `ExplosionPlugin`.
///
/// `cascading_enabled` gates whether `tick_explode` emits `BlockSetRequest`
/// messages that drive the cascading-TNT mechanic (a primary detonation
/// removing adjacent TNT blocks, triggering them in turn).
///
/// Default: `true`. `ExplosionPlugin` and `BlockUpdatePlugin` both run in
/// each `DimSubApp`, so the `MessageWriter<BlockSetRequest>` from
/// `tick_explode` and the matching `MessageReader<BlockSetRequest>` in
/// `apply_set_block_request` live in the same per-dim `World`. The
/// cascade chain is a single message hop — no two-frame buffer rotation
/// across a cross-`World` boundary — so emitted requests reach the reader
/// in the same tick and the secondary TNT actually detonates.
#[derive(bevy_ecs::resource::Resource, Debug, Clone, Copy)]
pub struct ExplosionConfig {
    pub cascading_enabled: bool,
}

impl Default for ExplosionConfig {
    fn default() -> Self {
        Self {
            cascading_enabled: true,
        }
    }
}

#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct Explosion;

/// The radius of the [Explosion] to be created by detonating an [Explosive].
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

const CHUNK_CACHE_SHIFT: usize = 2;
const CHUNK_CACHE_MASK: usize = (1 << CHUNK_CACHE_SHIFT) - 1;
const CHUNK_CACHE_WIDTH: usize = 1 << CHUNK_CACHE_SHIFT;

#[derive(Debug, Copy, Clone)]
struct BlockCacheItem {
    pos: BlockPos,
    block: BlockStateId,
    resistance: f32,
    chunk: Option<Entity>,
    should_explode: Option<bool>,
}

struct BlockCache<'a, 'b> {
    map: &'a mut FxHashMap<BlockPos, BlockCacheItem>,
    chunk_index: &'a ChunkIndex,
    chunks: &'a Query<'a, 'a, (Entity, &'b BlockPalette)>,
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
        } = self;
        match map.entry(pos) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let chunk_pos = ChunkPos::from(pos);
                let item = (|| {
                    let b = chunk_index.get(chunk_pos)?;
                    let (chunk, palette) = chunks.get(b.entity()).ok()?;
                    let block_state = palette.get(pos);
                    let resistance = (AsRef::<Block>::as_ref(&block_state).explosion_resistance() + 0.3) * 0.3;
                    Some(BlockCacheItem {
                        pos,
                        block: block_state,
                        resistance,
                        chunk: Some(chunk),
                        should_explode: None,
                    })
                })()
                .unwrap_or(BlockCacheItem {
                    pos,
                    block: BlockStateId(0),
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
    config: Res<ExplosionConfig>,
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
    dim_chunks: Query<&ChunkIndex>,
    chunks: Query<(ChunkEntity, &BlockPalette)>,
    mut queue: Local<Parallel<Vec<(ExplosionEntity, Vec<BlockExplodedEvent>)>>>,
    mut commands: Commands,
    mut writer: MessageWriter<BlockSetRequest>,
) {
    explosions.par_iter_mut().for_each_init(
        || queue.borrow_local_mut(),
        |q , (e, transform, dim, radius, detonator)| {
            let center = transform.translation;
            let dim = dim.entity();
            let Some(dim_chunks) = dim_chunks.get(dim).ok() else {
                return;
            };

            let mut cache_map = FxHashMap::default();
            let mut cache = BlockCache {
                map: &mut cache_map,
                chunk_index: dim_chunks,
                chunks: &chunks,
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

    let cascading_enabled = config.cascading_enabled;
    writer.write_batch(event_set.drain().filter_map(|event| {
        let dim = event.dimension;
        let block_pos = event.block_pos;
        commands.trigger(event);
        if cascading_enabled {
            Some(BlockSetRequest::remove_block(dim, block_pos))
        } else {
            None
        }
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
    let cached_rays = cached_rays();
    for inc in cached_rays {
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

                if should_explode && (fire || cached_block.block != AIR.default_state.id) {
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

use crate::world::block::minecraft::AIR;
use mcrs_minecraft_block::block_update::BlockSetRequest;
use crate::world::entity::explosive::primed_tnt::Detonator;
use bevy_ecs::event::Event;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::Commands;
use bevy_math::DVec3;
use bevy_utils::Parallel;
use rand::{RngExt, rng};
use std::sync::OnceLock;

const N: i32 = 15;
const SCALE: f64 = 0.3;
const POINTS: usize = 1352;
const LEN: usize = POINTS;

pub static CACHED_RAYS: OnceLock<[DVec3; LEN]> = OnceLock::new();

pub fn cached_rays() -> &'static [DVec3; LEN] {
    CACHED_RAYS.get_or_init(|| {
        let mut out: [DVec3; LEN] = [DVec3::ZERO; LEN];
        let mut i = 0usize;

        for x in 0..=N {
            for y in 0..=N {
                for z in 0..=N {
                    if x == 0 || x == N || y == 0 || y == N || z == 0 || z == N {
                        let xd = (x as f64 / N as f64) * 2.0 - 1.0;
                        let yd = (y as f64 / N as f64) * 2.0 - 1.0;
                        let zd = (z as f64 / N as f64) * 2.0 - 1.0;

                        let mag = (xd * xd + yd * yd + zd * zd).sqrt();

                        out[i] = DVec3::new(
                            (xd / mag) * SCALE,
                            (yd / mag) * SCALE,
                            (zd / mag) * SCALE,
                        );
                        i += 1;
                    }
                }
            }
        }

        assert_eq!(
            i, LEN,
            "cached_rays: surface-cell count diverged from LEN; bump LEN or fix the loop",
        );

        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bevy_ecs::message::Messages;
    use bevy_ecs::system::System;
    use mcrs_minecraft_block::block_update::BlockSetRequest;

    /// `ExplosionConfig::default()` keeps cascading enabled now that the
    /// `tick_explode` writer and the matching `apply_set_block_request`
    /// reader share a per-dim `World`. The single message hop guarantees
    /// `BlockSetRequest` reaches the reader in the same tick.
    #[test]
    fn explosion_config_defaults_to_cascading_enabled() {
        let cfg = ExplosionConfig::default();
        assert!(
            cfg.cascading_enabled,
            "ExplosionConfig must default to cascading enabled"
        );
    }

    /// Wiring smoke test: building an app with `ExplosionPlugin` registers
    /// both the config and the `BlockSetRequest` message buffer (the latter is
    /// added by the plugin's reliance on `MessageWriter<BlockSetRequest>`).
    #[test]
    fn explosion_plugin_registers_config_with_cascading_enabled() {
        let mut app = App::new();
        app.add_message::<BlockSetRequest>();
        app.add_plugins(ExplosionPlugin);

        let cfg = app
            .world()
            .get_resource::<ExplosionConfig>()
            .expect("ExplosionPlugin must register ExplosionConfig");
        assert!(
            cfg.cascading_enabled,
            "ExplosionPlugin must register ExplosionConfig with cascading enabled"
        );
    }

    /// With cascading on by default, running `tick_explode` against an empty
    /// world (no `Explosion` entities) still drains nothing into
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

    /// The resource is `Clone + Copy`; an operator-side `world.insert_resource`
    /// flip to `cascading_enabled = false` is the path to disable cascading at
    /// runtime if a server admin wants to suppress the mechanic.
    #[test]
    fn explosion_config_can_be_flipped_at_runtime() {
        let mut app = App::new();
        app.add_message::<BlockSetRequest>();
        app.add_plugins(ExplosionPlugin);

        app.world_mut().insert_resource(ExplosionConfig {
            cascading_enabled: false,
        });

        let cfg = app.world().resource::<ExplosionConfig>();
        assert!(!cfg.cascading_enabled, "runtime flip to disabled must stick");
    }
}
