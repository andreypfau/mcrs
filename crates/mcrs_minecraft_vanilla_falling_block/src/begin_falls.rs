use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Single;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion_replication::Appearance;
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::block_update::remove_block;
use mcrs_minecraft_engine_world::entity_births::EntityBirths;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::BirthSequences;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_protocol_game::block::BlockStateNumber;
use mcrs_minecraft_replication::SpawnLook;
use mcrs_voxel::BlockPos;

use crate::Falling;
use crate::PendingFall;

pub(crate) fn begin_falls(
    mut commands: Commands,
    pending: Query<(Entity, &PendingFall)>,
    dimension: Single<Entity, With<Dimension>>,
    births: EntityBirths,
    mut sequences: Local<BirthSequences>,
    mut blocks: MessageWriter<BlockSetRequest>,
) {
    for (entity, PendingFall(start)) in &pending {
        let falling_block = start.falling_block;
        let pos = BlockPos::from(falling_block.start);
        blocks.write(BlockSetRequest {
            new_state: start.leftover,
            ..remove_block(*dimension, pos)
        });
        let source = BirthSource::Block(pos);
        let key = births.born(&source, sequences.next(births.tick(), &source));
        let body = Body {
            position: start.position,
            ..Body::default()
        };
        let mut state = EntityState::new(key, body, None);
        state.phase = key.phase_offset();
        commands.entity(entity).remove::<PendingFall>().insert((
            MotionState(state),
            MotionKind(MotionRow::falling_block()),
            Falling(falling_block),
            Appearance {
                kind: &EntityTypeData::FALLING_BLOCK,
                look: SpawnLook::FallingBlock {
                    state: BlockStateNumber(falling_block.carried.number()),
                    start: pos,
                },
            },
        ));
    }
}
