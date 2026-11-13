use bevy_ecs::prelude::{Commands, Entity, Query, Res, ResMut, With};
use mcrs_minecraft_anvil::{ChunkStructures, SavedPiece};
use mcrs_minecraft_engine::stored_column::{PendingColumnFacts, SectionChanged, TakenFromStore};
use mcrs_minecraft_engine_world::world_seed::WorldSeed;
use mcrs_minecraft_worldgen_structure::piece::rebuild_monument_rooms;
use mcrs_voxel::{ColumnPos, SectionPos};
use tracing::warn;

/// A vanilla save keeps no room graph of an ocean monument, so a start that lands without one
/// gets the graph that generation makes from the seed. A rebuilt column counts as changed, so its
/// next save writes the graph.
pub(crate) fn rebuild_saved_monument_rooms(
    seed: Res<WorldSeed>,
    mut landed: ResMut<PendingColumnFacts>,
    sections: Query<(Entity, &SectionPos), With<TakenFromStore>>,
    mut commands: Commands,
) {
    for (&(_, column), (structures, _)) in landed.0.iter_mut() {
        if !rebuild_rooms(&mut structures.0, column, seed.0 as i64) {
            continue;
        }
        let section = sections
            .iter()
            .find_map(|(section, &pos)| (ColumnPos::from(pos) == column).then_some(section));
        match section {
            Some(section) => {
                commands.entity(section).try_insert(SectionChanged);
            }
            None => warn!(
                ?column,
                "a rebuilt monument column has no stored section to mark"
            ),
        }
    }
}

pub(crate) fn rebuild_rooms(
    structures: &mut ChunkStructures,
    column: ColumnPos,
    seed: i64,
) -> bool {
    let mut rebuilt = false;
    for start in structures.starts.values_mut() {
        for piece in &mut start.pieces {
            if let SavedPiece::OceanMonumentBuilding(tag) = piece
                && tag.rooms.is_empty()
            {
                rebuilt |= rebuild_monument_rooms(seed, column, tag);
            }
        }
    }
    rebuilt
}
