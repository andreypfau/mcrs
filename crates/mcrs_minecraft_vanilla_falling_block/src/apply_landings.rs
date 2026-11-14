use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::Single;
use glam::DVec3;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::column_change::ColumnChange;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::block_update::remove_block;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_falling_block::LandingEvent;
use mcrs_minecraft_falling_block::LandingOutcome;
use mcrs_minecraft_falling_block::Remains;
use mcrs_minecraft_physics::block_at;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLevelEvent;
use mcrs_voxel::BlockPos;
use mcrs_voxel::ColumnPos;

use crate::Falling;
use crate::Landed;
use crate::drop_item::drop_item;
use crate::falling_block_rules::FallingBlockRules;

/// Applies each landing that its review kept: the sets of the block, the drop and the level events,
/// then the removal of the falling block (FBK-LND-001 to FBK-LND-007).
pub(crate) fn apply_landings(
    mut commands: Commands,
    landed: Query<(Entity, &Landed, &Falling, &MotionState)>,
    dimension: Single<Entity, With<Dimension>>,
    rules: Res<FallingBlockRules>,
    definitions: Res<Blocks>,
    mut blocks: MessageWriter<BlockSetRequest>,
    mut changes: MessageWriter<ColumnChange>,
) {
    for (entity, Landed { landing, target }, Falling(falling_block), MotionState(motion)) in &landed
    {
        let position = motion.body.position;
        match landing.outcome {
            LandingOutcome::Wait => {
                commands.entity(entity).remove::<Landed>();
                continue;
            }
            LandingOutcome::Break | LandingOutcome::Item(Remains::Nothing) => {}
            LandingOutcome::Place(placement) => {
                // chisle: the block set reaches the viewers after the removal of the entity, in no
                // stated order, and with no `add_transient_block` (FBK-NET-002), so a viewer can
                // see a gap for one frame. A block packet in the event stream of the entity, or a
                // stated order in the muxer, lifts it.
                let set = |new_state| BlockSetRequest {
                    new_state,
                    ..remove_block(*dimension, *target)
                };
                blocks.write(set(placement.placed));
                if let Some(concrete) = placement.concrete {
                    blocks.write(set(concrete));
                }
            }
            LandingOutcome::Item(Remains::Item) => {
                let carried = falling_block.carried;
                drop_item(
                    &mut commands,
                    &rules,
                    &definitions,
                    carried,
                    position,
                    motion.key,
                );
            }
        }
        let centre = position + DVec3::Y * f64::from(EntityTypeData::FALLING_BLOCK.height) / 2.0;
        for event in landing.events() {
            let (pos, data) = match event {
                LandingEvent::BlockBreak => {
                    (block_at(centre), i32::from(falling_block.carried.number()))
                }
                LandingEvent::AnvilLand | LandingEvent::AnvilDestroy => (*target, 0),
                _ => continue,
            };
            changes.write(level_event(pos, event.id(), data));
        }
        commands.entity(entity).despawn();
    }
}

fn level_event(pos: BlockPos, effect: i32, effect_data: i32) -> ColumnChange {
    ColumnChange {
        column: ColumnPos::from(pos),
        payload: PacketPayload::LevelEvent(ClientboundLevelEvent {
            effect,
            pos,
            effect_data,
            global: false,
        }),
        except: None,
    }
}
