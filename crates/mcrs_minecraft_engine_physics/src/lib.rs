//! The entity pass of a dimension world in Bevy. The row store of
//! [`mcrs_minecraft_entity_rows`] is a resource, generic over the mob column that the AI of the
//! application supplies; systems lend it to the merge point, the freeze and the pass. Items and experience
//! orbs are rows only, with no ECS entity. Mobs are rows with a cold side table; the
//! changes of the tick wait in [`MobChanges`] for the systems of [`MobPublish`].

mod block_world;
mod entity_physics_plugin;
mod entity_writes;
mod mob_damage;
mod mob_life;
mod mob_spawn_request;
mod mobs;
mod row_damage;
#[cfg(feature = "testing")]
mod test_scene;

pub use entity_physics_plugin::EntityPass;
pub use entity_physics_plugin::EntityPhysicsPlugin;
pub use entity_physics_plugin::ItemStacks;
pub use entity_physics_plugin::MobPublish;
pub use entity_physics_plugin::PhysicsTables;
pub use entity_writes::EntityWrites;
pub use mob_damage::hurt_mob;
pub use mob_life::MobChanges;
pub use mob_life::Spawned;
pub use mob_life::Struck;
pub use mob_life::mob_body;
pub use mob_life::mob_row;
pub use mob_spawn_request::MobExtras;
pub use mob_spawn_request::MobSpawnRequest;
pub use mob_spawn_request::SpawnSetup;
pub use mobs::Mob;
pub use mobs::MobUuids;
pub use mobs::Mobs;
pub use row_damage::RowDamaged;
pub use row_damage::RowDied;
pub use row_damage::RowHit;
pub use row_damage::RowHitApply;
pub use row_damage::RowHits;
#[cfg(feature = "testing")]
pub use test_scene::FLOOR;
#[cfg(feature = "testing")]
pub use test_scene::TestScene;
