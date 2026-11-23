use bevy::ecs::message::Messages;
use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_client::test_support;
use mcrs_minecraft_client_entity::EntityDamaged;
use mcrs_minecraft_client_world::attack_input::AttackInput;
use mcrs_minecraft_client_world::block_breaking::BlockBreakingPlugin;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_client_world::local_motion::{self, LocalMotion};
use mcrs_minecraft_client_world::local_player::LocalPlayerPlugin;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_engine_world::entity::physics::{Rotation, Transform as PhysicsTransform};
use mcrs_minecraft_engine_world::world::storage::column::ColumnPos;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_visual::{ClientEntity, Motion};
use mcrs_minecraft_protocol::{Bounded, WritePacket};
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;

use crate::local_world::LocalWorld;

fn pig(world: &mut World) -> Option<(Entity, DVec3)> {
    world
        .query::<(Entity, &ClientEntity, &Motion)>()
        .iter(world)
        .find(|(_, client, _)| client.kind == EntityType::Pig)
        .map(|(entity, _, motion)| (entity, motion.current.position))
}

fn look_at(world: &mut World, point: DVec3) {
    let (mut transform, motion) = world
        .query_filtered::<(&mut PhysicsTransform, Option<&LocalMotion>), With<Player>>()
        .single_mut(world)
        .expect("one own player");
    let eye_height = f64::from(local_motion::eye_height(motion));
    let to = point - (transform.translation + DVec3::Y * eye_height);
    let yaw = (-to.x).atan2(to.z).to_degrees() as f32;
    let pitch = (-to.y).atan2(to.x.hypot(to.z)).to_degrees() as f32;
    transform.rotation = Rotation::new(yaw, pitch);
}

#[test]
fn an_attack_press_on_a_pig_sends_a_hit_that_the_server_applies() {
    let mut world = LocalWorld::join(
        "player-attack",
        |_| {},
        |app| {
            app.add_plugins((LocalPlayerPlugin, BlockBreakingPlugin));
        },
    );
    test_support::connection(world.app.world_mut()).write_packet(&ServerboundChatCommand {
        command: Bounded("summon minecraft:pig 0.5 0 2.5 {NoAI:1b}"),
    });
    let mut target = None;
    test_support::update_until(&mut world.app, |app| {
        target = pig(app.world_mut());
        let column_held = app
            .world()
            .resource::<ColumnStore>()
            .holds(ColumnPos::from(DVec3::ZERO));
        target.is_some() && column_held
    });
    let (pig, _) = target.expect("the pig arrived");
    test_support::update_until(&mut world.app, |app| {
        let world = app.world_mut();
        if let Some((_, position)) = self::pig(world) {
            look_at(world, position + DVec3::Y * 0.45);
        }
        world.resource_mut::<AttackInput>().presses = 1;
        world.run_schedule(FixedUpdate);
        world
            .resource_mut::<Messages<EntityDamaged>>()
            .drain()
            .any(|damaged| damaged.entity == pig)
    });
    world.leave();
}
