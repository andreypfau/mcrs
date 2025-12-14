use crate::world::block::BlockState;
use crate::world::entity::explosive::Explosive;
use crate::world::palette::BlockPalette;
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::entity::{ContainsEntity, Entity};
use bevy_ecs::query::With;
use bevy_ecs::system::{Local, Query};
use bevy_reflect::Reflect;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{ChunkIndex, ChunkPos};
use mcrs_engine::world::dimension::InDimension;
use mcrs_protocol::BlockStateId;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::mem::MaybeUninit;

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
#[derive(Component, Reflect, Default, Debug)]
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
    map: FxHashMap<BlockPos, BlockCacheItem>,
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
                    let resistance = (block_state.as_ref().explosion_resistance() + 0.3) * 0.3;
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
        |q, (e, transform, dim, radius, detonator)| {
            let center = transform.translation;
            let dim = dim.entity();
            let Some(dim_chunks) = dim_chunks.get(dim).ok() else {
                return;
            };
            let _span = tracing::info_span!("tick_explode::calc_blocks").entered();
            let blocks = calc_blocks(
                dim,
                center,
                radius.0 as f32,
                &mut rng(),
                false,
                detonator.map(|d| d.entity()),
                dim_chunks,
                &chunks,
            );
            drop(_span);
            q.push((e, blocks));
        },
    );

    let _span = tracing::info_span!("tick_explode::deduplicate blocks").entered();
    let mut event_set = FxHashSet::<BlockExplodedEvent>::default();
    for (explosion, events) in queue.drain() {
        commands.entity(explosion).despawn();
        for event in events {
            event_set.insert(event);
        }
    }
    drop(_span);

    writer.write_batch(event_set.drain().map(|event| {
        let dim = event.dimension;
        let block_pos = event.block_pos;
        commands.trigger(event);
        BlockSetRequest::remove_block(dim, block_pos)
    }));
}

fn calc_blocks<R>(
    dimension: DimEntity,
    center: DVec3,
    radius: f32,
    random: &mut R,
    fire: bool,
    detonator: Option<Entity>,
    dim_chunks: &ChunkIndex,
    chunks: &Query<(ChunkEntity, &BlockPalette)>,
) -> Vec<BlockExplodedEvent>
where
    R: rand::Rng,
{
    let mut cache = BlockCache {
        map: FxHashMap::default(),
        chunk_index: dim_chunks,
        chunks: &chunks,
    };

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
            curr = curr + inc;
            if power <= 0.0 {
                break;
            }
        }
    }

    ret
}

use crate::world::block::minecraft::AIR;
use crate::world::block_update::BlockSetRequest;
use crate::world::entity::explosive::primed_tnt::Detonator;
use bevy_ecs::event::Event;
use bevy_ecs::message::{Message, MessageWriter};
use bevy_ecs::prelude::{Commands, On};
use bevy_math::DVec3;
use bevy_math::ops::exp;
use bevy_utils::Parallel;
use rand::{Rng, rng};
use std::sync::OnceLock;

const N: i32 = 15;
const SCALE: f64 = 0.3;
const POINTS: usize = 1352;
const LEN: usize = POINTS;

pub static CACHED_RAYS: OnceLock<[DVec3; LEN]> = OnceLock::new();

pub fn cached_rays() -> &'static [DVec3; LEN] {
    CACHED_RAYS.get_or_init(|| {
        let mut out: [MaybeUninit<DVec3>; LEN] = [MaybeUninit::uninit(); LEN];
        let mut i = 0usize;

        for x in 0..=N {
            for y in 0..=N {
                for z in 0..=N {
                    if x == 0 || x == N || y == 0 || y == N || z == 0 || z == N {
                        let xd = (x as f64 / N as f64) * 2.0 - 1.0;
                        let yd = (y as f64 / N as f64) * 2.0 - 1.0;
                        let zd = (z as f64 / N as f64) * 2.0 - 1.0;

                        let mag = (xd * xd + yd * yd + zd * zd).sqrt();

                        out[i].write(DVec3::new(
                            (xd / mag) * SCALE,
                            (yd / mag) * SCALE,
                            (zd / mag) * SCALE,
                        ));
                        i += 1;
                    }
                }
            }
        }

        debug_assert_eq!(i, LEN);

        // SAFETY: все элементы [0..LEN) записаны ровно один раз.
        unsafe { std::mem::transmute::<[MaybeUninit<DVec3>; LEN], [DVec3; LEN]>(out) }
    })
}
