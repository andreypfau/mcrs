//! The entity pass of a dimension world in Bevy. The row store of
//! [`mcrs_minecraft_entity_rows`] is a resource, with the hot state of each mob with AI in its
//! mob column; systems lend it to the merge point, the freeze and the pass. Items and experience
//! orbs are rows only, with no ECS entity. Mobs are rows with a cold side table, and publish
//! their tile events for the muxer.

mod block_world;
mod entity_physics_plugin;
mod entity_rows;
mod entity_writes;
mod mob_column;
mod mob_damage;
mod mob_life;
mod mob_publish;
mod mob_spawn_request;
mod mobs;
#[cfg(feature = "testing")]
mod test_scene;
mod tile_at;

pub use entity_physics_plugin::EntityPhysicsPlugin;
pub use entity_physics_plugin::ItemStacks;
pub use entity_physics_plugin::PhysicsTables;
pub use entity_rows::EntityRows;
pub use entity_writes::EntityWrites;
pub use mob_column::MobColumn;
pub use mob_damage::hurt_mob;
pub use mob_life::MobChanges;
pub use mob_life::mob_body;
pub use mob_life::mob_row;
pub use mob_spawn_request::MobExtras;
pub use mob_spawn_request::MobSpawnRequest;
pub use mob_spawn_request::SpawnSetup;
pub use mobs::Mob;
pub use mobs::MobUuids;
pub use mobs::Mobs;
#[cfg(feature = "testing")]
pub use test_scene::FLOOR;
#[cfg(feature = "testing")]
pub use test_scene::TestScene;
pub use tile_at::tile_at;
