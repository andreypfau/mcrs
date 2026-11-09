use mcrs_minecraft_brain::PlayerRow;
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_physics::CollisionTable;

use crate::PanicCauses;

/// The read-only inputs of the hot side of one AI tick.
#[derive(Debug)]
pub struct HotTick<'a, V> {
    pub blocks: &'a V,
    pub collision: &'a CollisionTable,
    pub mobs: &'a Mobs,
    pub panic_causes: &'a PanicCauses,
    /// The players of the dimension at the end of the last tick.
    pub players: &'a [PlayerRow],
}
