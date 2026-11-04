use std::collections::vec_deque::Drain;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Component, Has, Local, ParamSet, Query, Res, ResMut, Resource};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_engine_link::{Link, Placement};
use mcrs_engine_store::{
    ChunkVersion, NoDisk, StoreInput, StoreOutput, StoreStage, VersionedChunk,
};
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine_world::palette::{BiomePalette, ChunkBlocks};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::lifecycle::stage::{
    SectionStage, SectionStageChanged, SectionStages,
};
use mcrs_minecraft_engine_world::world::lifecycle::ticket::{
    ChunkSpawnSet, ColumnCoolSet, SectionEntitySet,
};
use mcrs_minecraft_engine_world::world::light_gate::{
    ColumnLightGate, ColumnLightSettled, LightGated,
};
use mcrs_minecraft_engine_world::world::storage::block_entity::{
    BlockEntity, SectionBlockEntities, spawn_block_entities,
};
use mcrs_minecraft_engine_world::world::storage::column::{
    ColumnIndex, ColumnSections, SectionLookup,
};
use mcrs_minecraft_heightmap::{
    ColumnHeightmapSet, MotionHeightmap, NoLeavesHeightmap, SolidHeightmap, SurfaceHeightmap,
};
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashMap;

use crate::heightmap::PendingColumnHeightmaps;
use crate::stored_column::{StoredColumn, StoredSection};

/// The disk under the data layer of a dimension. The data layer lives in memory only for now.
pub type ColumnDisk = NoDisk<ColumnPos, StoredColumn>;

/// The data layer of a dimension world, at the far end of a link: the last version of each
/// column that went cold.
///
/// A column that stops being loaded writes its last version here, and a column that a ticket
/// loads again is taken from here before it is generated.
pub struct ColumnStorePlugin {
    pub placement: Placement,
}

/// The near end of the link to the data layer of a dimension world.
#[derive(Resource)]
pub struct ColumnStoreLink {
    link: Link<StoreStage<ColumnDisk>>,
    /// The version of the last column the world wrote. The world is the only writer.
    written: ChunkVersion,
    /// The columns whose sections wait for an answer of the data layer.
    reads: FxHashMap<ColumnPos, ColumnRead>,
}

struct ColumnRead {
    dim: Entity,
    sections: Vec<Entity>,
}

impl ColumnStoreLink {
    #[must_use]
    pub fn new(placement: Placement) -> Self {
        Self {
            link: Link::new(StoreStage::new(ColumnDisk::new()), placement),
            written: ChunkVersion::default(),
            reads: FxHashMap::default(),
        }
    }

    /// Waits until the data layer has answered every request sent so far, and lands none of it.
    pub async fn answered(&mut self) {
        self.link.answered().await;
    }

    fn put(&mut self, column: ColumnPos, chunk: StoredColumn) {
        self.written = self.written.next();
        self.link.send(StoreInput::Put {
            key: column,
            chunk: VersionedChunk {
                version: self.written,
                chunk,
            },
        });
    }

    /// Asks for `column` on behalf of `section`, once for all the sections that wait on it.
    fn get(&mut self, dim: Entity, column: ColumnPos, section: Entity) {
        let read = self.reads.entry(column).or_insert_with(|| {
            self.link.send(StoreInput::Get { key: column });
            ColumnRead {
                dim,
                sections: Vec::new(),
            }
        });
        read.sections.push(section);
    }

    fn outputs(&mut self) -> Drain<'_, StoreOutput<ColumnPos, StoredColumn>> {
        self.link.outputs()
    }
}

/// Where the sections that the data layer handed back land, at the start of the tick. Other
/// sources of section content land after it, so the order of the landings is fixed.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoredColumnLandSet;

/// The light of the column, and of the eight around it, settled while it was loaded, so the
/// light it holds is the light it keeps when it goes cold.
#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct ColumnLightFinal;

impl Plugin for ColumnStorePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ColumnStoreLink::new(self.placement));
        app.add_systems(
            FixedUpdate,
            (
                land_stored_columns
                    .in_set(TickSet::DomainInput)
                    .in_set(StoredColumnLandSet),
                write_cold_columns.in_set(ColumnCoolSet),
                fetch_loading_columns
                    .in_set(ChunkSpawnSet)
                    .after(SectionEntitySet),
                mark_final_light
                    .in_set(TickSet::Project)
                    .after(ColumnLightGate),
            ),
        );
    }
}

/// Sends the data layer a request for each column whose sections a ticket loads, before they
/// are generated. A section that the data layer did not hold comes back as `Loading` and goes
/// on to generation.
fn fetch_loading_columns(
    mut stages: ParamSet<(MessageReader<SectionStageChanged>, SectionStages)>,
    mut loading: Local<Vec<SectionStageChanged>>,
    mut store: ResMut<ColumnStoreLink>,
) {
    loading.extend(stages.p0().read().filter(|change| {
        change.to == SectionStage::Loading && change.from != Some(SectionStage::Fetching)
    }));
    let mut stages = stages.p1();
    for change in loading.drain(..) {
        if stages.get(change.section) != Some(SectionStage::Loading) {
            continue;
        }
        stages.set(change.section, SectionStage::Fetching);
        store.get(change.dim, ColumnPos::from(change.pos), change.section);
    }
}

fn land_stored_columns(
    mut store: ResMut<ColumnStoreLink>,
    mut stages: SectionStages,
    positions: Query<&SectionPos>,
    mut pending_heightmaps: ResMut<PendingColumnHeightmaps>,
    mut commands: Commands,
    mut answers: Local<Vec<StoreOutput<ColumnPos, StoredColumn>>>,
) {
    answers.extend(store.outputs());
    for answer in answers.drain(..) {
        let (column, stored) = match answer {
            StoreOutput::Found { key, chunk } => (key, Some(chunk.chunk)),
            // The data layer lives in memory only, so it never fails to read.
            StoreOutput::Missing { key } | StoreOutput::Unreadable { key } => (key, None),
        };
        let Some(read) = store.reads.remove(&column) else {
            continue;
        };
        let Some(stored) = stored else {
            for section in read.sections {
                if stages.get(section) == Some(SectionStage::Fetching) {
                    stages.set(section, SectionStage::Loading);
                }
            }
            continue;
        };
        let mut landed_rows: Vec<i32> = Vec::new();
        for section in read.sections {
            if stages.get(section) != Some(SectionStage::Fetching) {
                continue;
            }
            let Some(content) = positions
                .get(section)
                .ok()
                .and_then(|pos| stored.section(pos.y).map(|content| (pos.y, content)))
            else {
                stages.set(section, SectionStage::Loading);
                continue;
            };
            let (y, content) = content;
            let mut entity = commands.entity(section);
            entity.try_insert((content.blocks.clone(), content.biomes.clone()));
            if let Some(light) = &content.block_light {
                entity.try_insert(light.clone());
            }
            if let Some(light) = &content.sky_light {
                entity.try_insert(light.clone());
            }
            stages.set(section, SectionStage::Loaded);
            landed_rows.push(y);
        }
        let block_entities = stored
            .block_entities
            .iter()
            .filter(|entry| landed_rows.contains(&(entry.position().y >> SectionPos::BITS)))
            .cloned()
            .collect();
        spawn_block_entities(&mut commands, InDimension(read.dim), block_entities);
        if let Some(maps) = stored.heightmaps {
            pending_heightmaps.0.insert((read.dim, column), maps);
        }
    }
}

fn mark_final_light(
    mut settled: MessageReader<ColumnLightSettled>,
    indices: Query<&ColumnIndex>,
    mut commands: Commands,
) {
    for &ColumnLightSettled(column) in settled.read() {
        for index in &indices {
            if let Some(slot) = index.get(&column) {
                commands.entity(slot.entity).try_insert(ColumnLightFinal);
            }
        }
    }
}

type SectionContent = (
    &'static SectionStage,
    &'static ChunkBlocks,
    &'static BiomePalette,
    Option<&'static BlockLight>,
    Option<&'static SkyLight>,
    Option<&'static SectionBlockEntities>,
);

type ColumnContent = (
    &'static ColumnSections,
    Has<ColumnLightFinal>,
    Option<(
        &'static SurfaceHeightmap,
        &'static SolidHeightmap,
        &'static MotionHeightmap,
        &'static NoLeavesHeightmap,
    )>,
);

/// Writes the last version of each column that stopped being loaded, while its sections still
/// hold their blocks. A column that never landed whole, or whose light never settled, keeps
/// the version the data layer already holds.
fn write_cold_columns(
    dims: Query<(&ColumnLoading, &ColumnIndex)>,
    columns: Query<ColumnContent>,
    sections: Query<SectionContent>,
    block_entities: Query<&BlockEntity>,
    light_gated: Option<Res<LightGated>>,
    mut store: ResMut<ColumnStoreLink>,
) {
    for (loading, index) in &dims {
        for &column in loading.cooling() {
            if loading.is_column_loaded(column) {
                continue;
            }
            let Some(slot) = index.get(&column) else {
                continue;
            };
            let Ok((column_sections, light_final, heightmaps)) = columns.get(slot.entity) else {
                continue;
            };
            if light_gated.is_some() && !light_final {
                continue;
            }
            let Some(stored) = stored_column(column_sections, &sections, &block_entities) else {
                continue;
            };
            store.put(
                column,
                StoredColumn {
                    heightmaps: heightmaps.map(|(surface, solid, motion, no_leaves)| {
                        ColumnHeightmapSet {
                            surface: surface.clone(),
                            solid: solid.clone(),
                            motion: motion.clone(),
                            no_leaves: no_leaves.clone(),
                        }
                    }),
                    ..stored
                },
            );
        }
    }
}

/// The sections and block entities of a column, or `None` when a section has not landed.
fn stored_column(
    column: &ColumnSections,
    sections: &Query<SectionContent>,
    block_entities: &Query<&BlockEntity>,
) -> Option<StoredColumn> {
    let mut stored = StoredColumn {
        min_section_y: column.min_section_y,
        sections: Vec::with_capacity(column.sections.len()),
        block_entities: Vec::new(),
        heightmaps: None,
    };
    for lookup in column.iter() {
        let SectionLookup::Loaded(section) = lookup else {
            return None;
        };
        let (stage, blocks, biomes, block_light, sky_light, held) = sections.get(section).ok()?;
        if *stage != SectionStage::Loaded {
            return None;
        }
        stored.sections.push(StoredSection {
            blocks: blocks.clone(),
            biomes: biomes.clone(),
            block_light: block_light.cloned(),
            sky_light: sky_light.cloned(),
        });
        stored.block_entities.extend(
            held.into_iter()
                .flat_map(|held| held.iter())
                .filter_map(|entity| block_entities.get(*entity).ok())
                .map(|BlockEntity(entry)| entry.clone()),
        );
    }
    Some(stored)
}
