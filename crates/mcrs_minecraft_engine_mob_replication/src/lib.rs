//! The replication projection of the mobs of a dimension world: at the end of each tick it reads
//! the rows that the pass marked as moved, the tile moves and the mob changes, and writes the
//! tile events of the mobs for the muxer. It keeps no copy of the state of a mob.

mod mob_replication_plugin;
mod mob_state;
mod publish_mobs;

pub use mob_replication_plugin::MobReplicationPlugin;
