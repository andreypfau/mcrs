use bevy_ecs::change_detection::Mut;
use bevy_ecs::world::World;
use mcrs_minecraft_command_engine::Worlds;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_engine_brain::EntityRows;
use mcrs_minecraft_engine_physics::{RowMobChanges, hurt_row};
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_entity_rows::Placed;

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
    let outcome = world
        .try_resource_scope(|world, mut rows: Mut<EntityRows>| {
            let mut changes = world.get_resource_mut::<RowMobChanges>()?;
            hurt_row(&mut rows, &mut changes, at, hit, tick)
        })
        .flatten();
    if outcome.is_none() {
        tracing::debug!(?at, "a command hit finds no mob row; it does nothing");
    }
}
