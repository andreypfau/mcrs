use bevy_ecs::change_detection::Mut;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_command_engine::Worlds;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_engine_physics::MobChanges;
use mcrs_minecraft_engine_physics::hurt_mob;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;

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
    let outcome = world
        .try_resource_scope(|world, mut changes: Mut<MobChanges>| {
            let mut mob = world.get_entity_mut(target).ok()?;
            if !mob.contains::<Mob>() {
                return None;
            }
            let mut state = mob.get_mut::<MotionState>()?;
            hurt_mob(target, &mut state.0, &mut changes, hit, tick)
        })
        .flatten();
    if outcome.is_none() {
        tracing::debug!(?target, "a command hit finds no mob; it does nothing");
    }
}
