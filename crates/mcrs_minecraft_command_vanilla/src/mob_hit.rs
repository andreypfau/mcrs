use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_command_engine::Worlds;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_engine_damage::DamageRequest;
use mcrs_minecraft_engine_damage::PendingHits;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_entity_key::WriterKey;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitMob {
    pub target: Entity,
    pub hit: Hit,
}

pub(crate) fn apply(worlds: &mut Worlds, HitMob { target, hit }: HitMob) {
    let world: &mut World = worlds.world_mut();
    let tick = world
        .get_resource::<BirthClock>()
        .map_or(0, |clock| clock.0);
    let Some(mut pending) = world.get_resource_mut::<PendingHits>() else {
        tracing::debug!(?target, "a command hit finds no hit queue; it does nothing");
        return;
    };
    let sequence = u32::try_from(pending.len()).unwrap_or(u32::MAX);
    pending.push(DamageRequest::new(
        target,
        hit,
        WriterKey::Command { sequence: 0 },
        tick,
        sequence,
    ));
}
