use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Component, Local, ParamSet, Query, ResMut};
use mcrs_engine_store::{ChunkVersion, StoreOutput};
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::stage::{
    SectionStage, SectionStageChanged, SectionStages,
};
use mcrs_minecraft_engine_world::world::light_gate::SentLight;
use mcrs_minecraft_engine_world::world::storage::block_entity::spawn_block_entities;
use mcrs_voxel::{ColumnPos, SectionPos};

use crate::column_asks::ColumnOrigin;
use crate::column_store::{ColumnRead, ColumnStoreLink, FetchedColumn, Produced, ReadFrom};
use crate::column_streams::ColumnStreams;
use crate::column_ticks::LandedTicks;
use crate::heightmap::PendingColumnHeightmaps;
use crate::stored_column::{StoredColumn, StoredSection};

/// The section came back from the data layer, so the data layer already holds its blocks.
#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct TakenFromStore;

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
        store.take(change.dim, column, change.section);
    }
}

/// Lands each answer of the data layer: in the sections that wait to take the column back, and
/// in the answers that the production of cold columns waits on.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub(crate) fn land_stored_columns(
    mut store: ResMut<ColumnStoreLink>,
    mut stages: SectionStages,
    positions: Query<&SectionPos>,
    mut pending_heightmaps: ResMut<PendingColumnHeightmaps>,
    streams: Query<&ColumnStreams>,
    mut commands: Commands,
    mut answers: Local<Vec<StoreOutput<ColumnPos, StoredColumn>>>,
    mut landed_ticks: Option<ResMut<LandedTicks>>,
) {
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
        if missing && matches!(answer, StoreOutput::Unreadable { .. }) {
            store.mark_unreadable(column);
        }
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
                    dim: read.dim,
                    column,
                    sections: &read.sections,
                    from: read.from,
                    held: match streams.get(read.dim) {
                        Ok(streams) if streams.is_subscribed(column) => Held::ByViews,
                        _ => Held::ByNone,
                    },
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
    column: ColumnPos,
    sections: &'a [Entity],
    from: ReadFrom,
    held: Held,
}

/// Whether views hold the column that lands, with the light that the data layer stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Held {
    ByViews,
    ByNone,
}

/// Puts the blocks of `stored` into the sections that wait for them, with its block entities,
/// heightmaps and scheduled ticks. The dimension lights the sections itself, since the data layer
/// may light the version again at any time and what the dimension simulates must not depend on
/// when that happened. The stored light stays only as the [`SentLight`] of the views that hold it.
fn land_column(
    landing: Landing<'_>,
    stored: StoredColumn,
    stages: &mut SectionStages,
    positions: &Query<&SectionPos>,
    commands: &mut Commands,
    pending_heightmaps: &mut PendingColumnHeightmaps,
    landed_ticks: Option<&mut LandedTicks>,
) {
    let StoredColumn {
        min_section_y,
        sections,
        block_entities,
        heightmaps,
        mut ticks,
        ..
    } = stored;
    let mut contents: Vec<Option<StoredSection>> = sections.into_iter().map(Some).collect();
    let mut landed_rows: Vec<i32> = Vec::new();
    for &section in landing.sections {
        if stages.get(section) != Some(SectionStage::Fetching) {
            continue;
        }
        let Some(y) = positions.get(section).ok().map(|pos| pos.y) else {
            continue;
        };
        let content = y
            .checked_sub(min_section_y)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| contents.get_mut(index))
            .and_then(Option::take);
        let Some(content) = content else {
            stages.set(section, SectionStage::Loading);
            continue;
        };
        let mut entity = commands.entity(section);
        entity.try_insert((content.blocks, content.biomes));
        if landing.held == Held::ByViews
            && (content.block_light.is_some() || content.sky_light.is_some())
        {
            entity.try_insert(SentLight {
                block: content.block_light.map(|light| light.0),
                sky: content.sky_light.map(|light| light.0),
            });
        }
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
    if let Some(landed_ticks) = landed_ticks {
        ticks.retain_rows(&landed_rows);
        landed_ticks.land(landing.column, ticks);
    }
    if let Some(maps) = heightmaps
        && !landing.sections.is_empty()
    {
        pending_heightmaps
            .0
            .insert((landing.dim, landing.column), maps);
    }
}
