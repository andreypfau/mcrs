use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{
    Commands, Component, Has, Local, ParamSet, Query, ResMut, Resource, Single, With,
};
use mcrs_engine_store::{ChunkVersion, StoreOutput};
use mcrs_minecraft_engine_world::world::dimension::{Dimension, InDimension};
use mcrs_minecraft_engine_world::world::lifecycle::simulation::EntityTicking;
use mcrs_minecraft_engine_world::world::lifecycle::stage::{
    SectionStage, SectionStageChanged, SectionStages,
};
use mcrs_minecraft_engine_world::world::storage::block_entity::spawn_block_entities;
use mcrs_minecraft_engine_world::world::storage::column::ColumnIndex;
use mcrs_minecraft_engine_world::world::storage::random_ticks::RandomTickIndex;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashMap;

use crate::column_asks::ColumnOrigin;
use crate::column_store::{ColumnRead, ColumnStoreLink, FetchedColumn, Produced, ReadFrom};
use crate::column_ticks::LandedTicks;
use crate::heightmap::PendingColumnHeightmaps;
use crate::stored_column::{ColumnRecord, ColumnStructures, RawBlockEntities};
use mcrs_minecraft_region_disk::stored_column::{StoredColumn, StoredSection};

/// The section came back from the data layer, so the data layer already holds its blocks.
#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct TakenFromStore;

/// Column facts of a landed column, waiting for the column entity to exist.
#[derive(Resource, Default, Debug)]
pub struct PendingColumnFacts(pub FxHashMap<(Entity, ColumnPos), (ColumnStructures, ColumnRecord)>);

/// Puts the facts of each landed column on its column entity, once `reconcile_columns` made it.
pub(crate) fn attach_column_facts(
    mut pending: ResMut<PendingColumnFacts>,
    indices: Query<&ColumnIndex>,
    mut commands: Commands,
) {
    for ((dim, column), (structures, extras)) in pending.0.drain() {
        let Some(slot) = indices.get(dim).ok().and_then(|index| index.get(&column)) else {
            continue;
        };
        commands
            .entity(slot.entity)
            .try_insert((structures, extras));
    }
}

/// Sends the data layer a request for each column whose sections a ticket loads, before they
/// are generated. A section that the data layer did not hold comes back as `Loading` and goes
/// on to generation.
pub(crate) fn fetch_loading_columns(
    mut stages: ParamSet<(MessageReader<SectionStageChanged>, SectionStages)>,
    mut loading: Local<Vec<SectionStageChanged>>,
    mut store: ResMut<ColumnStoreLink>,
) {
    loading.extend(stages.p0().read().filter(|change| {
        change.to == SectionStage::Loading && change.from != Some(SectionStage::Fetching)
    }));
    let mut stages = stages.p1();
    for change in loading.drain(..) {
        let column = ColumnPos::from(change.pos);
        if (store.origin(column) == ColumnOrigin::Generation && !store.generates())
            || stages.get(change.section) != Some(SectionStage::Loading)
        {
            continue;
        }
        stages.set(change.section, SectionStage::Fetching);
        store.take(column, change.section);
    }
}

/// Lands each answer of the data layer: in the sections that wait to take the column back, and
/// in the answers that the production of cold columns waits on.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub(crate) fn land_stored_columns(
    dim: Single<(Entity, &mut SectionIndex), With<Dimension>>,
    mut random_ticks: Option<ResMut<RandomTickIndex>>,
    mut store: ResMut<ColumnStoreLink>,
    mut stages: SectionStages,
    positions: Query<(&SectionPos, Has<EntityTicking>)>,
    mut pending_heightmaps: ResMut<PendingColumnHeightmaps>,
    mut pending_facts: ResMut<PendingColumnFacts>,
    mut commands: Commands,
    mut answers: Local<Vec<StoreOutput<ColumnPos, StoredColumn>>>,
    mut landed_ticks: Option<ResMut<LandedTicks>>,
) {
    let (dim, section_index) = dim.into_inner();
    let section_index = section_index.into_inner();
    answers.extend(store.outputs());
    for answer in answers.drain(..) {
        let (column, version) = match &answer {
            StoreOutput::Found { key, chunk } => (*key, chunk.version),
            StoreOutput::Missing { key } | StoreOutput::Unreadable { key } => {
                (*key, ChunkVersion::default())
            }
        };
        if store
            .reads
            .get(&column)
            .is_none_or(|read| version < read.at_least)
        {
            continue;
        }
        let missing = !matches!(answer, StoreOutput::Found { .. });
        // A column that neither its owner nor the data layer holds is generated.
        if missing
            && store
                .reads
                .get(&column)
                .is_some_and(|read| read.from == ReadFrom::Store)
            && store.read_generated(column)
        {
            continue;
        }
        let Some(read) = store.reads.remove(&column) else {
            continue;
        };
        let answer = match answer {
            StoreOutput::Found { key, chunk } => {
                // The production of cold columns keeps a copy; else the sections take the column.
                let kept = match read.produced {
                    Produced::Yes => Some(chunk.clone()),
                    Produced::No => None,
                };
                let landing = Landing {
                    dim,
                    index: &mut *section_index,
                    random_ticks: random_ticks.as_deref_mut(),
                    facts: &mut pending_facts,
                    column,
                    sections: &read.sections,
                    from: read.from,
                };
                land_column(
                    landing,
                    chunk.chunk,
                    &mut stages,
                    &positions,
                    &mut commands,
                    &mut pending_heightmaps,
                    landed_ticks.as_deref_mut(),
                );
                kept.map(|chunk| StoreOutput::Found { key, chunk })
            }
            unheld @ (StoreOutput::Missing { .. } | StoreOutput::Unreadable { .. }) => {
                drop_sections(&read, &mut stages);
                Some(unheld)
            }
        };
        if read.produced == Produced::Yes
            && let Some(answer) = answer
        {
            store.fetched.push(FetchedColumn {
                column,
                answer,
                from: read.from,
            });
        }
    }
}

/// Sends the sections of a read that found no column back to loading. The generation drops a
/// column only once it left every view, so then the sections go too.
fn drop_sections(read: &ColumnRead, stages: &mut SectionStages) {
    let next = match read.from {
        ReadFrom::Store => SectionStage::Loading,
        ReadFrom::Generation => SectionStage::Unloading,
    };
    for &section in &read.sections {
        if stages.get(section) == Some(SectionStage::Fetching) {
            stages.set(section, next);
        }
    }
}

/// The sections of one column that wait for an answer, and where the answer came from.
struct Landing<'a> {
    dim: Entity,
    index: &'a mut SectionIndex,
    random_ticks: Option<&'a mut RandomTickIndex>,
    facts: &'a mut PendingColumnFacts,
    column: ColumnPos,
    sections: &'a [Entity],
    from: ReadFrom,
}

/// Puts the blocks of `stored` into the sections that wait for them, with its block entities,
/// heightmaps and scheduled ticks. The dimension lights the sections itself, since the data layer
/// may light the version again at any time and what the dimension simulates must not depend on
/// when that happened.
fn land_column(
    mut landing: Landing<'_>,
    stored: StoredColumn,
    stages: &mut SectionStages,
    positions: &Query<(&SectionPos, Has<EntityTicking>)>,
    commands: &mut Commands,
    pending_heightmaps: &mut PendingColumnHeightmaps,
    landed_ticks: Option<&mut LandedTicks>,
) {
    let StoredColumn {
        min_section_y,
        sections,
        block_entities,
        unknown_block_entities,
        heightmaps,
        structures,
        inhabited_time,
        last_update,
        retrogen,
        post_processing,
        upgrade_data,
        blending_data,
        mut ticks,
        ..
    } = stored;
    let mut contents: Vec<Option<StoredSection>> = sections.into_iter().map(Some).collect();
    let mut landed_rows: Vec<i32> = Vec::new();
    for &section in landing.sections {
        if stages.get(section) != Some(SectionStage::Fetching) {
            continue;
        }
        let Ok((&pos, random_ticked)) = positions.get(section) else {
            continue;
        };
        let y = pos.y;
        let content = y
            .checked_sub(min_section_y)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| contents.get_mut(index))
            .and_then(Option::take);
        let Some(content) = content else {
            stages.set(section, SectionStage::Loading);
            continue;
        };
        let random_ticks = landing.random_ticks.as_deref_mut();
        if !landing
            .index
            .land(pos, section, content.blocks, random_ticks, random_ticked)
        {
            stages.set(section, SectionStage::Loading);
            continue;
        }
        let mut entity = commands.entity(section);
        entity.try_insert(content.biomes);
        if landing.from == ReadFrom::Store {
            entity.try_insert(TakenFromStore);
        }
        stages.set(section, SectionStage::Loaded);
        landed_rows.push(y);
    }
    let block_entities = block_entities
        .into_iter()
        .filter(|entry| landed_rows.contains(&(entry.position().y >> SectionPos::BITS)))
        .collect();
    spawn_block_entities(commands, InDimension(landing.dim), block_entities);
    let mut raw_by_row: std::collections::BTreeMap<i32, Vec<_>> = std::collections::BTreeMap::new();
    for entry in unknown_block_entities {
        if let Some(y) = entry.get_int("y") {
            raw_by_row
                .entry(y >> SectionPos::BITS)
                .or_default()
                .push(entry);
        }
    }
    for &section in landing.sections {
        let Ok((&pos, _)) = positions.get(section) else {
            continue;
        };
        if let Some(raw) = raw_by_row.remove(&pos.y) {
            commands.entity(section).try_insert(RawBlockEntities(raw));
        }
    }
    if !landed_rows.is_empty() {
        landing.facts.0.insert(
            (landing.dim, landing.column),
            (
                ColumnStructures(structures),
                ColumnRecord {
                    inhabited_time,
                    last_update,
                    retrogen,
                    post_processing,
                    upgrade_data,
                    blending_data,
                },
            ),
        );
    }
    if let Some(landed_ticks) = landed_ticks {
        ticks.retain_rows(&landed_rows);
        landed_ticks.land(landing.column, ticks);
    }
    if !landing.sections.is_empty() {
        pending_heightmaps
            .0
            .insert((landing.dim, landing.column), heightmaps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use mcrs_minecraft_resource::ResourceLocationBuf;

    use bevy_ecs::message::Messages;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use mcrs_engine_store::VersionedChunk;

    use mcrs_minecraft_region_disk::column_ticks::ColumnTicks;

    fn empty_column() -> StoredColumn {
        StoredColumn {
            status: mcrs_minecraft_anvil::ChunkStatus::Full,
            is_light_on: false,
            inhabited_time: 0,
            last_update: 0,
            min_section_y: 0,
            sections: Vec::new(),
            block_entities: Vec::new(),
            unknown_block_entities: Vec::new(),
            heightmaps: mcrs_minecraft_heightmap::ColumnHeightmapSet::new(0, 0),
            retrogen: None,
            structures: Default::default(),
            post_processing: Vec::new(),
            upgrade_data: Default::default(),
            blending_data: None,
            ticks: ColumnTicks::default(),
        }
    }

    #[test]
    fn column_facts_live_once_on_the_column_and_survive_the_snapshot() {
        use crate::column_snapshot::ColumnSnapshots;
        use mcrs_minecraft_anvil::{ChunkStructures, RetroGen};
        use mcrs_minecraft_engine_world::palette::{BiomePalette, ChunkBlocks, SectionStates};
        use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
        use mcrs_minecraft_engine_world::world::storage::column::reconcile_columns;
        use mcrs_voxel::BlockStateId;

        const ROWS: i32 = 3;
        let column = ColumnPos::new(1, 2);
        let mut world = World::new();
        world.init_resource::<Messages<SectionStageChanged>>();
        world.init_resource::<PendingColumnHeightmaps>();
        world.init_resource::<PendingColumnFacts>();
        let mut index = SectionIndex::new();
        let dim = world.spawn_empty().id();
        let sections: Vec<Entity> = (0..ROWS)
            .map(|y| {
                let pos = SectionPos::new(column.x, y, column.z);
                let section = world
                    .spawn((SectionStage::Fetching, pos, InDimension(dim)))
                    .id();
                index.insert(pos, section);
                section
            })
            .collect();
        world.entity_mut(dim).insert((
            Dimension,
            index,
            ColumnIndex::default(),
            DimensionTypeConfig::new(0, 16 * ROWS as u32),
        ));

        let mut stored = empty_column();
        stored.sections = (0..ROWS)
            .map(|_| StoredSection {
                blocks: ChunkBlocks::new(SectionStates::homogeneous(BlockStateId::AIR)),
                biomes: BiomePalette::default(),
                block_light: None,
                sky_light: None,
            })
            .collect();
        stored.inhabited_time = 77;
        stored.retrogen = Some(RetroGen {
            target_status: mcrs_minecraft_anvil::ChunkStatus::Full,
            statuses_to_rerun: Vec::new(),
            has_below_zero_retrogen: false,
            missing_bedrock: Vec::new(),
        });
        stored.structures = ChunkStructures {
            references: [(
                ResourceLocationBuf::read("minecraft:village").unwrap(),
                [ColumnPos::new(0, 0)].into(),
            )]
            .into(),
            ..Default::default()
        };
        let expected_structures = stored.structures.clone();

        let mut store = ColumnStoreLink::new(None);
        for &section in &sections {
            store.take(column, section);
        }
        store.receive(vec![StoreOutput::Found {
            key: column,
            chunk: VersionedChunk {
                version: ChunkVersion::default(),
                chunk: stored,
            },
        }]);
        world.insert_resource(store);
        world.run_system_once(land_stored_columns).unwrap();
        world.run_system_once(reconcile_columns).unwrap();
        world.run_system_once(attach_column_facts).unwrap();

        assert_eq!(world.query::<&ColumnStructures>().iter(&world).count(), 1);
        assert_eq!(world.query::<&ColumnRecord>().iter(&world).count(), 1);
        let entity = world.get::<ColumnIndex>(dim).unwrap()[&column].entity;
        let maps = mcrs_minecraft_heightmap::ColumnHeightmapSet::new(0, 0);
        world
            .entity_mut(entity)
            .insert((maps.surface, maps.solid, maps.motion, maps.no_leaves));
        let snapshot = world
            .run_system_once(move |snapshots: ColumnSnapshots| snapshots.take(entity))
            .unwrap()
            .expect("the column is whole");
        assert_eq!(snapshot.structures, expected_structures);
        assert_eq!(snapshot.inhabited_time, 77);
        assert!(snapshot.retrogen.is_some());
        assert_eq!(snapshot.sections.len(), ROWS as usize);
    }
}
