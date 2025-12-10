use crate::direction::{Direction, DirectionSet};
use crate::world::chunk_tickets::ChunkTicketsPlugin;
use crate::world::generate::generate_chunk;
use crate::world::paletted_container::PalettedContainer;
use crate::world::pumpkin_palette::{BiomePalette, BlockPalette};
use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Plugin};
use bevy_ecs::entity::{Entity, EntityHashSet};
use bevy_ecs::prelude::{Bundle, Commands, Component, Name, Query, Resource};
use bevy_ecs::query::Changed;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::ResMut;
use bevy_reflect::Reflect;
use bevy_tasks::futures_lite::future;
use bevy_tasks::{Task, TaskPool, TaskPoolBuilder, block_on};
use mcrs_protocol::math::IVec3;
use mcrs_protocol::{BlockPos, BlockStateId, CellPos, ChunkPos, Position};
use rustc_hash::FxHashMap;
use std::sync::OnceLock;

pub struct ChunkPlugin;

impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        CHUNK_TASK_POOL.get_or_init(|| {
            TaskPoolBuilder::new()
                .thread_name("ChunkGen".to_string())
                .num_threads(4)
                .build()
        });
        app.insert_resource(ChunkIndex::default());
        app.insert_resource(LoadingChunks::default());
        app.add_plugins(ChunkTicketsPlugin);
        app.add_systems(
            FixedPreUpdate,
            (load_chunks, process_generated_chunk).chain(),
        );
        app.add_systems(FixedPostUpdate, unload_chunks);
    }
}

static CHUNK_TASK_POOL: OnceLock<TaskPool> = OnceLock::new();

#[derive(Resource, Default, Debug)]
struct LoadingChunks(FxHashMap<ChunkPos, Task<ChunkLoadingTask>>);

struct ChunkLoadingTask {
    entity: Entity,
    pos: ChunkPos,
    block_states: ChunkBlockStates,
    biomes: BiomesChunk,
}

fn unload_chunks(
    mut query: Query<(Entity, &ChunkStatus, &ChunkPos), Changed<ChunkStatus>>,
    mut chunk_index: ResMut<ChunkIndex>,
    mut commands: Commands,
) {
    for (entity, status, pos) in query.iter_mut() {
        if *status == ChunkStatus::Unloaded {
            commands.entity(entity).despawn();
            chunk_index.remove(pos);
            println!(
                "Found Unloaded chunk at {:?} {:?}, despawning...",
                pos, entity
            );
        }
    }
}

fn load_chunks(
    mut query: Query<(Entity, &mut ChunkStatus, &ChunkPos)>,
    mut loading_chunks: ResMut<LoadingChunks>,
) {
    let task_pool = CHUNK_TASK_POOL.get().unwrap();
    query.iter_mut().for_each(|(e, mut status, pos)| {
        if *status != ChunkStatus::Loading {
            return;
        }
        *status = ChunkStatus::Generating;
        let pos = *pos;
        let task = task_pool.spawn(async move {
            let mut block_states = ChunkBlockStates::default();
            let mut biomes = BiomesChunk::default();
            generate_chunk(pos, &mut block_states, &mut biomes);
            ChunkLoadingTask {
                entity: e,
                pos,
                block_states,
                biomes,
            }
        });
        loading_chunks.0.insert(pos, task);
    })
}

fn process_generated_chunk(
    mut loading_chunks: ResMut<LoadingChunks>,
    mut query: Query<(&mut ChunkStatus, &mut ChunkBlockStates, &mut BiomesChunk)>,
) {
    loading_chunks.0.retain(|pos, task| {
        let res = block_on(future::poll_once(task));
        let retain = res.is_none();
        if let Some(loaded_chunk) = res {
            let Ok((mut status, mut block_states, mut biomes)) = query.get_mut(loaded_chunk.entity)
            else {
                return false;
            };
            *status = ChunkStatus::Ready;
            *block_states = loaded_chunk.block_states;
            *biomes = loaded_chunk.biomes;
        }
        retain
    })
}

#[derive(Component, Default, Clone)]
pub struct ChunkBlockStates(pub BlockPalette);

impl ChunkBlockStates {
    pub fn fill<B: Into<BlockStateId>>(&mut self, block: B) {
        self.0 = BlockPalette::Homogeneous(block.into());
    }

    pub fn get<I: Into<BlockPos>>(&self, pos: I) -> BlockStateId {
        let pos = pos.into();
        self.0.get(
            pos.x as usize & ChunkPos::MASK,
            pos.y as usize & ChunkPos::MASK,
            pos.z as usize & ChunkPos::MASK,
        )
    }

    pub fn set<I: Into<BlockPos>, B: Into<BlockStateId>>(
        &mut self,
        pos: I,
        block: B,
    ) -> BlockStateId {
        let pos = pos.into();
        self.0.set(
            pos.x as usize & ChunkPos::MASK,
            pos.y as usize & ChunkPos::MASK,
            pos.z as usize & ChunkPos::MASK,
            block.into(),
        )
    }

    pub(super) fn count_non_air_blocks(&self) -> u16 {
        self.0.non_air_block_count()
    }
}

#[derive(Component, Default, Clone)]
pub struct BiomesChunk(pub BiomePalette);

#[derive(Bundle, Default)]
pub struct ChunkBundle {
    pub block_states: ChunkBlockStates,
    pub biomes: BiomesChunk,
    pub status: ChunkStatus,
    pub chunk_pos: ChunkPos,
    pub name: Name,
}

impl ChunkBundle {
    pub fn new(chunk_pos: ChunkPos) -> Self {
        Self {
            chunk_pos,
            name: Name::new(format!(
                "chunk::{}:{}:{}",
                chunk_pos.x, chunk_pos.y, chunk_pos.z
            )),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Component, Reflect)]
pub enum ChunkStatus {
    Unloaded,
    #[default]
    Loading,
    Generating,
    Ready,
}

#[derive(Resource, Debug, Default)]
pub struct ChunkIndex(FxHashMap<ChunkPos, ChunkIndexEntry>);

#[derive(Debug, Clone)]
pub struct ChunkIndexEntry {
    pub chunk: Entity,
    pub neighbours: DirectionSet,
    pub entities: EntityHashSet,
    pub observers: EntityHashSet,
}

impl PartialEq for ChunkIndexEntry {
    fn eq(&self, other: &Self) -> bool {
        self.chunk == other.chunk
    }
}


impl ChunkIndex {
    pub fn contains(&self, pos: &ChunkPos) -> bool {
        self.0.contains_key(pos)
    }

    pub fn get<P : Into<ChunkPos>>(&self, pos: P) -> Option<&ChunkIndexEntry> {
        self.0.get(&pos.into())
    }
    
    pub fn get_mut<P : Into<ChunkPos>>(&mut self, pos: P) -> Option<&mut ChunkIndexEntry> {
        self.0.get_mut(&pos.into())
    }

    pub fn insert(&mut self, pos: ChunkPos, entity: Entity) -> Option<ChunkIndexEntry> {
        // println!("ChunkIndex inserting chunk index entry for {pos:?}");
        let mut self_directions = DirectionSet::default();
        for direction in Direction::all() {
            let neighbour_pos = pos + IVec3::from(direction);
            if let Some(neighbour_directions) = self.0.get_mut(&neighbour_pos) {
                neighbour_directions.neighbours.insert(direction.opposite());
                self_directions.insert(direction);
            }
        }
        self.0.insert(
            pos,
            ChunkIndexEntry {
                chunk: entity,
                neighbours: self_directions,
                entities: EntityHashSet::default(),
                observers: EntityHashSet::default(),
            },
        )
    }

    pub fn remove(&mut self, pos: &ChunkPos) -> Option<ChunkIndexEntry> {
        let entry = self.0.remove(pos);
        if let Some(entry) = entry {
            // println!("ChunkIndex removing chunk index entry for {pos:?}");
            for direction in entry.neighbours {
                let neighbour_pos = *pos + IVec3::from(direction);
                if let Some(neighbour) = self.0.get_mut(&neighbour_pos) {
                    neighbour.neighbours.remove(direction.opposite());
                }
            }
            Some(entry)
        } else {
            None
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ChunkPos, &ChunkIndexEntry)> {
        self.0.iter()
    }
}
