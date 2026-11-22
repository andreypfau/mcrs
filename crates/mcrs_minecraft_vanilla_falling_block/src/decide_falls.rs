use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::Single;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion::StepOutcome;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_falling_block::AfterMove;
use mcrs_minecraft_falling_block::EntityDrops;
use mcrs_minecraft_falling_block::Fall;
use mcrs_minecraft_falling_block::Ground;
use mcrs_minecraft_falling_block::LandingOutcome;
use mcrs_minecraft_falling_block::LandingSite;
use mcrs_minecraft_falling_block::Remains;
use mcrs_minecraft_falling_block::Replaceable;
use mcrs_minecraft_falling_block::Silence;
use mcrs_minecraft_falling_block::TargetCell;
use mcrs_minecraft_falling_block::WorldHeight;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::block_at;
use mcrs_minecraft_vanilla_block_place::Spot;
use mcrs_minecraft_vanilla_block_place::replaceable;
use mcrs_minecraft_volume::BlockVolume;
use mcrs_minecraft_volume::Unavailable;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

use crate::Falling;
use crate::Landed;
use crate::drop_item::drop_item;
use crate::falling_block_rules::FallingBlockRules;

#[derive(Clone, Copy, Debug)]
struct Touchdown {
    entity: Entity,
    fall: Fall,
    cell: TargetCell,
}

pub(crate) fn decide_falls(
    mut commands: Commands,
    falls: Query<(Entity, &Falling, &MotionState, &StepOutcome)>,
    dimension: Single<(Entity, &DimensionTypeConfig), With<Dimension>>,
    world: WorldView,
    rules: Res<FallingBlockRules>,
    definitions: Res<Blocks>,
    clock: Res<BirthClock>,
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
    let tick = Tick {
        rules: &rules,
        definitions: &definitions,
        state_at,
        height: WorldHeight {
            min_y: config.min_y,
            height: config.height,
        },
        entity_drops: entity_drops(game_rules.as_deref()),
    };
    for (entity, Falling(fall), MotionState(motion), outcome) in &falls {
        if outcome.is_of(clock.0)
            && let Some(touchdown) = tick.after_move(&mut commands, entity, fall, motion, outcome)
        {
            tick.land(&mut commands, touchdown);
        }
    }
}

pub(crate) fn entity_drops(game_rules: Option<&GameRules>) -> EntityDrops {
    if game_rules.is_none_or(|rules| rules.is_on(GameRule::EntityDrops)) {
        EntityDrops::On
    } else {
        EntityDrops::Off
    }
}

struct Tick<'a, S> {
    rules: &'a FallingBlockRules,
    definitions: &'a BlockDefinitions,
    state_at: S,
    height: WorldHeight,
    entity_drops: EntityDrops,
}

impl<S: Fn(BlockPos) -> Option<BlockStateId> + Copy> Tick<'_, S> {
    fn after_move(
        &self,
        commands: &mut Commands,
        entity: Entity,
        fall: &Fall,
        motion: &EntityState,
        outcome: &StepOutcome,
    ) -> Option<Touchdown> {
        let body = motion.body;
        let cell = TargetCell::find(
            &self.rules.table,
            self.definitions,
            self.state_at,
            fall,
            outcome.start,
            body.position,
            body.velocity,
        );
        let ground = if body.flags.on_ground {
            Ground::OnGround
        } else {
            Ground::Airborne
        };
        let after = fall.after_move(
            motion.age,
            self.rules.settings,
            ground,
            cell.water,
            block_at(body.position).y,
            self.height,
            self.entity_drops,
        );
        match after {
            AfterMove::Fall => None,
            AfterMove::Expire(remains) => {
                if remains == Remains::Item {
                    drop_item(
                        commands,
                        self.rules,
                        self.definitions,
                        fall.carried,
                        body.position,
                        motion.key,
                    );
                }
                commands.entity(entity).despawn();
                None
            }
            AfterMove::Land => Some(Touchdown {
                entity,
                fall: *fall,
                cell,
            }),
        }
    }

    fn land(&self, commands: &mut Commands, touchdown: Touchdown) {
        let table = &self.rules.table;
        let target = touchdown.cell.pos;
        let replaceable_at = |state| {
            if replaceable(self.definitions, state, Block::Barrier.id(), Spot::Adjacent) {
                Replaceable::Replaceable
            } else {
                Replaceable::Occupied
            }
        };
        let Some(site) = LandingSite::read(
            table,
            self.definitions,
            self.state_at,
            touchdown.cell,
            replaceable_at,
        ) else {
            return;
        };
        let landing = touchdown
            .fall
            .land(table, &site, self.entity_drops, Silence::Audible);
        if landing.outcome != LandingOutcome::Wait {
            commands
                .entity(touchdown.entity)
                .insert(Landed { landing, target });
        }
    }
}
