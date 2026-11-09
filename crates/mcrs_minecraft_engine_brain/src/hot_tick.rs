use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_physics::CollisionTable;

/// The read-only inputs of the hot side of one AI tick.
#[derive(Debug)]
pub struct HotTick<'a, V> {
    pub blocks: &'a V,
    pub collision: &'a CollisionTable,
    pub mobs: &'a Mobs,
}
