//! The entity pass of a dimension world. The hot state of the server entities lives in rows by
//! tile; each tick steps every row with the physics kernel, in parallel over tiles, against the
//! blocks of the last tick and the body grid from the start of the tick. Items and experience
//! orbs are rows only, with no ECS entity. Mobs are rows with a cold side table, and publish
//! their tile events for the muxer.

mod block_world;
mod entity_pass;
mod entity_physics_plugin;
mod entity_rows;
mod entity_write;
mod front_grid;
mod mob_damage;
mod mob_intent;
mod mob_life;
mod mob_moved;
mod mob_publish;
mod mob_spawn_request;
mod mobs;
#[cfg(feature = "testing")]
mod test_scene;
mod tile_at;
mod tile_world;

pub use entity_physics_plugin::EntityPhysicsPlugin;
pub use entity_physics_plugin::ItemStacks;
pub use entity_rows::EntityRows;
pub use entity_write::EntityWrites;
pub use mob_damage::hurt_mob;
pub use mob_intent::MobIntent;
pub use mob_life::MobChanges;
pub use mob_life::Placed;
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
