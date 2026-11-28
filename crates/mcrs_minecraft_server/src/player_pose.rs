use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{DetectChangesMut, Local, Query, Res, Single, With};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::resource_exists;
use mcrs_minecraft_collision_table::BlockWorld;
use mcrs_minecraft_engine::ability::PlayerFlying;
use mcrs_minecraft_engine::{PoseSystems, ShiftKey, player_scale};
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::view::WorldView;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_physics::{NoBodies, PoseInputs, QueryEntity, Scratch, View, select_pose};
use mcrs_minecraft_pose::Pose;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PlayerPosePlugin;

impl Plugin for PlayerPosePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            select_player_poses
                .run_if(resource_exists::<PhysicsTables>)
                .in_set(PoseSystems::Select),
        );
    }
}

fn select_player_poses(
    dimension: Single<Entity, With<Dimension>>,
    world: WorldView,
    tables: Res<PhysicsTables>,
    mut players: Query<
        (
            &Transform,
            &ShiftKey,
            &PlayerFlying,
            &PlayerGameMode,
            Option<&AttributeInstances>,
            &mut Pose,
        ),
        With<Player>,
    >,
    mut scratch: Local<Scratch>,
) {
    let Some(blocks) = world.get(*dimension) else {
        return;
    };
    let block_world = BlockWorld {
        blocks: &blocks,
        table: tables.collision(),
    };
    let view = View::new(&block_world, &NoBodies, tables.tables());
    for (transform, &ShiftKey(shift), &PlayerFlying(flight), mode, attributes, mut pose) in
        &mut players
    {
        let inputs = PoseInputs {
            current: *pose,
            entity: QueryEntity {
                key: EntityUuid::default(),
                position: transform.translation,
                fall_distance: 0.0,
                shift,
                powder_snow_solid: false,
                walks_on_powder_snow: false,
            },
            scale: player_scale(attributes),
            sleeping: false,
            swimming: false,
            fall_flying: false,
            spin_attack: false,
            shift,
            flight,
            spectator: mode.0 == GameMode::Spectator,
            passenger: false,
        };
        pose.set_if_neq(select_pose(view, &inputs, &mut scratch));
    }
}
