use bevy_ecs::world::World;
use mcrs_minecraft_command_engine::Worlds;
use mcrs_minecraft_engine::coordinator::birth_clock::BirthClock;
use mcrs_minecraft_engine_physics::{EntityRows, Hit, MobChanges, Mobs, Placed, hurt};

/// The command effect that hits the mob row `at` of this world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitMob {
    pub at: Placed,
    pub hit: Hit,
}

pub(crate) fn apply(worlds: &mut Worlds, HitMob { at, hit }: HitMob) {
    let world: &mut World = worlds.world_mut();
    let tick = world
        .get_resource::<BirthClock>()
        .map_or(0, |clock| clock.0);
    world.resource_scope(
        |world, mut rows: bevy_ecs::change_detection::Mut<EntityRows>| {
            world.resource_scope(|world, mut mobs: bevy_ecs::change_detection::Mut<Mobs>| {
                let Some(mut changes) = world.get_resource_mut::<MobChanges>() else {
                    return;
                };
                hurt(&mut rows, &mut mobs, &mut changes, at, hit, tick);
            });
        },
    );
}
