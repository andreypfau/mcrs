use bevy_ecs::prelude::*;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::block_tag_keys;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_entity_visual::{SleepingBed, SyncedData};
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::BlockStateId;

#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub(crate) struct BedFacings(Vec<Option<Direction>>);

impl BedFacings {
    fn of(&self, state: BlockStateId) -> Option<Direction> {
        self.0.get(usize::from(u16::from(state))).copied().flatten()
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::per_frame_insert_resource,
        reason = "the system runs only when the registries change, and the new facings replace the old ones"
    )
)]
pub(crate) fn build_bed_facings(
    mut commands: Commands,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
) {
    let mut facings = vec![None; blocks.state_count()];
    let beds = registries
        .tags::<Block>()
        .and_then(|tags| Some((tags.id(block_tag_keys::BEDS)?, tags)));
    if let Some((beds, tags)) = beds {
        for state in blocks.states() {
            if !blocks
                .block_index(state)
                .is_some_and(|block| tags.contains(beds, block))
            {
                continue;
            }
            let facing: Option<Direction> = blocks
                .owner(state)
                .and_then(|entry| entry.value_of(state, "facing"))
                .and_then(|value| {
                    let name = value.to_text();
                    Direction::HORIZONTAL
                        .into_iter()
                        .find(|direction| direction.name() == name)
                });
            if let Some(slot) = facings.get_mut(usize::from(u16::from(state))) {
                *slot = facing;
            }
        }
    }
    commands.insert_resource(BedFacings(facings));
}

pub(crate) fn find_sleeping_beds(
    columns: Res<ColumnStore>,
    facings: Res<BedFacings>,
    mut players: Query<(&SyncedData, &mut SleepingBed)>,
) {
    for (data, mut bed) in &mut players {
        let facing = data
            .sleeping_position
            .and_then(|at| columns.block(at).ok())
            .and_then(|state| facings.of(state));
        bed.set_if_neq(SleepingBed(facing));
    }
}
