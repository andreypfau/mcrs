use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::Single;
use glam::IVec3;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion::StepOutcome;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_falling_block::AfterMove;
use mcrs_minecraft_falling_block::EntityDrops;
use mcrs_minecraft_falling_block::FallingBlockTable;
use mcrs_minecraft_falling_block::Ground;
use mcrs_minecraft_falling_block::LandingOutcome;
use mcrs_minecraft_falling_block::LandingSite;
use mcrs_minecraft_falling_block::Remains;
use mcrs_minecraft_falling_block::Replaceable;
use mcrs_minecraft_falling_block::Silence;
use mcrs_minecraft_falling_block::Support;
use mcrs_minecraft_falling_block::WaterBeside;
use mcrs_minecraft_falling_block::WaterContact;
use mcrs_minecraft_falling_block::WorldHeight;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_physics::block_at;
use mcrs_minecraft_vanilla_block_place::Spot;
use mcrs_minecraft_vanilla_block_place::replaceable;
use mcrs_minecraft_volume::BlockVolume;
use mcrs_minecraft_volume::Unavailable;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

use crate::FallingBlockEntity;
use crate::Landed;
use crate::drop_item::drop_item;
use crate::falling_block_rules::FallingBlockRules;
use crate::water_contact_at::Contact;
use crate::water_contact_at::water_contact_at;

/// The neighbours of a target cell whose water hardens a concrete powder (FBK-LND-004).
const WATER_SIDES: [IVec3; 5] = [IVec3::Y, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];

/// Steps 4 to 6 of FBK-TCK-001 after the motion pass: a falling block that lands gets its
/// [`Landed`], one that a time limit ends drops and is removed (FBK-TCK-004, FBK-TCK-005).
pub(crate) fn decide_falls(
    mut commands: Commands,
    falls: Query<(Entity, &FallingBlockEntity, &MotionState, &StepOutcome)>,
    dimension: Single<(Entity, &DimensionTypeConfig), With<Dimension>>,
    world: WorldView,
    rules: Res<FallingBlockRules>,
    definitions: Res<Blocks>,
    game_rules: Option<Res<GameRules>>,
) {
    let (dimension, config) = *dimension;
    let Some(view) = world.get(dimension) else {
        return;
    };
    let void_air = definitions.default_state_of(Block::VoidAir.id());
    let state_at = |pos: BlockPos| match view.block(pos) {
        Ok(state) => Some(state),
        Err(Unavailable::Outside) => Some(void_air),
        Err(_) => None,
    };
    let height = WorldHeight {
        min_y: config.min_y,
        height: config.height,
    };
    let entity_drops = if game_rules.is_none_or(|rules| rules.is_on(GameRule::EntityDrops)) {
        EntityDrops::On
    } else {
        EntityDrops::Off
    };
    let table = &rules.table;
    for (entity, FallingBlockEntity(falling_block), MotionState(motion), outcome) in &falls {
        let body = motion.body;
        let contact = if table.is_concrete_powder(falling_block.carried) {
            water_contact_at(
                table,
                &definitions,
                state_at,
                outcome.start,
                body.position,
                body.velocity,
            )
        } else {
            Contact {
                water: WaterContact::Dry,
                target: block_at(body.position),
            }
        };
        let ground = if body.flags.on_ground {
            Ground::OnGround
        } else {
            Ground::Airborne
        };
        let after = falling_block.after_move(
            rules.settings,
            ground,
            contact.water,
            block_at(body.position).y,
            height,
            entity_drops,
        );
        match after {
            AfterMove::Fall => {}
            AfterMove::Expire(remains) => {
                if remains == Remains::Item {
                    let carried = falling_block.carried;
                    drop_item(
                        &mut commands,
                        &rules,
                        &definitions,
                        carried,
                        body.position,
                        motion.key,
                    );
                }
                commands.entity(entity).despawn();
            }
            AfterMove::Land => {
                let Some(site) = landing_site(table, &definitions, state_at, contact) else {
                    continue;
                };
                let landing = falling_block.land(table, &site, entity_drops, Silence::Audible);
                if landing.outcome != LandingOutcome::Wait {
                    commands.entity(entity).insert(Landed {
                        landing,
                        target: contact.target,
                    });
                }
            }
        }
    }
}

/// What a landing reads at the target cell, or `None` when a block it reads is not loaded.
// chisle: each carried state stays by its support predicate, and only a neighbour with no
// collision shape counts as water beside, so a waterlogged neighbour with an open face does not
// harden a powder. The support predicate of SUP-PRED-001 and the face test of SUP-FACE-001 lift it.
fn landing_site(
    table: &FallingBlockTable,
    definitions: &BlockDefinitions,
    state_at: impl Fn(BlockPos) -> Option<BlockStateId>,
    contact: Contact,
) -> Option<LandingSite> {
    let target = state_at(contact.target)?;
    let below = state_at(contact.target - IVec3::Y)?;
    // An empty hand: the barrier is never replaceable, so it matches no replaceable target.
    let replaceable = if replaceable(definitions, target, Block::Barrier.id(), Spot::Adjacent) {
        Replaceable::Replaceable
    } else {
        Replaceable::Occupied
    };
    let open_water = |offset: IVec3| {
        state_at(contact.target + offset).is_some_and(|state| {
            table.holds_water(state)
                && definitions
                    .shape(definitions.state(state).collision_shape)
                    .is_empty()
        })
    };
    let water_beside = if WATER_SIDES.into_iter().any(open_water) {
        WaterBeside::Present
    } else {
        WaterBeside::Absent
    };
    Some(LandingSite {
        target,
        below,
        replaceable,
        support: Support::Stays,
        water_contact: contact.water,
        water_beside,
    })
}
