//! The vanilla falling block: a requested fall becomes an entity that moves with the motion pass
//! of the engine and lands as a block or drops as an item (`docs/spec/falling-block.md`).

mod apply_landings;
mod begin_falls;
mod begin_ticks;
mod decide_falls;
mod drop_item;
mod fall_request;
mod falling_block_entity;
mod falling_block_plugin;
mod falling_block_rules;
mod landed;
mod pack_concrete_pairs;
mod pending_fall;
mod water_contact_at;

pub use fall_request::FallRequest;
pub use falling_block_entity::FallingBlockEntity;
pub use falling_block_plugin::FallingBlockPlugin;
pub use falling_block_plugin::FallingBlockSet;
pub use landed::Landed;
pub use pack_concrete_pairs::PackConcretePairs;
pub use pending_fall::PendingFall;
