//! The entity pass of a dimension world. The hot state of the server entities lives in rows by
//! tile; each tick steps every row with the physics kernel, in parallel over tiles, against the
//! blocks of the last tick and the body grid from the start of the tick. Items and experience
//! orbs are rows only, with no ECS entity.

mod block_world;
mod entity_pass;
mod entity_physics_plugin;
mod entity_rows;
mod entity_write;
mod front_grid;
#[cfg(feature = "testing")]
mod test_scene;
mod tile_at;
mod tile_world;

pub use entity_physics_plugin::EntityPhysicsPlugin;
pub use entity_physics_plugin::ItemStacks;
pub use entity_rows::EntityRows;
pub use entity_write::EntityWrites;
#[cfg(feature = "testing")]
pub use test_scene::FLOOR;
#[cfg(feature = "testing")]
pub use test_scene::TestScene;
