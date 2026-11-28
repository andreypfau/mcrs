use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Local;
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
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_falling_block::LandingEvent;
use mcrs_minecraft_falling_block::LandingOutcome;
use mcrs_minecraft_falling_block::LandingSite;
use mcrs_minecraft_falling_block::Remains;
use mcrs_minecraft_falling_block::Replaceable;
use mcrs_minecraft_falling_block::Silence;
use mcrs_minecraft_falling_block::Support;
use mcrs_minecraft_falling_block::WaterBeside;
use mcrs_minecraft_falling_block::WaterContact;
use mcrs_minecraft_physics::block_at;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLevelEvent;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::ColumnPos;

use crate::Falling;
use crate::Landed;
use crate::decide_falls::entity_drops;
use crate::drop_item::drop_item;
use crate::falling_block_rules::FallingBlockRules;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Kept {
    target: BlockPos,
    key: EntityUuid,
    entity: Entity,
}

#[derive(Clone, Copy, Debug)]
struct Claim {
    pos: BlockPos,
    state: BlockStateId,
}

impl Claim {
    fn site(self) -> LandingSite {
        LandingSite {
            target: self.state,
            below: self.state,
            replaceable: Replaceable::Occupied,
            support: Support::Stays,
            water_contact: WaterContact::Dry,
            water_beside: WaterBeside::Absent,
        }
    }
}

pub(crate) fn apply_landings(
    mut commands: Commands,
    landed: Query<(Entity, &Landed, &Falling, &MotionState)>,
    dimension: Single<Entity, With<Dimension>>,
    rules: Res<FallingBlockRules>,
    definitions: Res<Blocks>,
    game_rules: Option<Res<GameRules>>,
    mut blocks: MessageWriter<BlockSetRequest>,
    mut changes: MessageWriter<ColumnChange>,
    mut kept: Local<Vec<Kept>>,
) {
    kept.clear();
    for (entity, Landed { target, .. }, _, MotionState(motion)) in &landed {
        kept.push(Kept {
            target: *target,
            key: motion.key,
            entity,
        });
    }
    kept.sort_unstable_by_key(|kept| (kept.target.to_array(), kept.key));
    let entity_drops = entity_drops(game_rules.as_deref());
    let mut claim: Option<Claim> = None;
    for Kept { entity, .. } in kept.iter() {
        let Ok((entity, Landed { landing, target }, Falling(falling_block), MotionState(motion))) =
            landed.get(*entity)
        else {
            continue;
        };
        let landing = match (landing.outcome, claim) {
            (LandingOutcome::Place(_), Some(claim)) if claim.pos == *target => {
                falling_block.land(&rules.table, &claim.site(), entity_drops, Silence::Audible)
            }
            _ => *landing,
        };
        let position = motion.body.position;
        match landing.outcome {
            LandingOutcome::Wait => {
                commands.entity(entity).remove::<Landed>();
                continue;
            }
            LandingOutcome::Break | LandingOutcome::Item(Remains::Nothing) => {}
            LandingOutcome::Place(placement) => {
                let set = |new_state| BlockSetRequest {
                    new_state,
                    ..remove_block(*dimension, *target)
                };
                blocks.write(set(placement.placed));
                if let Some(concrete) = placement.concrete {
                    blocks.write(set(concrete));
                }
                claim = Some(Claim {
                    pos: *target,
                    state: placement.concrete.unwrap_or(placement.placed),
                });
            }
            LandingOutcome::Item(Remains::Item) => {
                let carried = falling_block.carried;
                drop_item(
                    commands.reborrow(),
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
