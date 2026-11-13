use bevy_ecs::change_detection::Mut;
use bevy_ecs::world::World;
use mcrs_minecraft_combat::DamageTypeRulesResource;
use mcrs_minecraft_command_engine::Worlds;
use mcrs_minecraft_damage::{Defence, Hit};
use mcrs_minecraft_engine_brain::EntityRows;
use mcrs_minecraft_engine_physics::{MobChanges, hurt_mob};
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_entity_rows::Placed;

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
    let flags = world
        .get_resource::<DamageTypeRulesResource>()
        .map_or_else(Default::default, |rules| rules.0.flags(hit.damage_type));
    let defence = Defence::new(flags);
    let outcome = world
        .try_resource_scope(|world, mut rows: Mut<EntityRows>| {
            let mut changes = world.get_resource_mut::<MobChanges>()?;
            hurt_mob(&mut rows, &mut changes, at, hit, &defence, tick)
        })
        .flatten();
    if outcome.is_none() {
        tracing::debug!(?at, "a command hit finds no mob row; it does nothing");
    }
}
