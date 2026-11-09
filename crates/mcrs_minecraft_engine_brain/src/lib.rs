//! The AI stage of the mobs of a dimension world: the host side that publishes the snapshot of
//! the mob rows, applies the decisions through the mob controls and steers the rows before the
//! entity pass, and the stage that runs per tile beside the pass.

mod ai_stats;
mod block_goal_world;
mod host_mob;
mod hot_tick;
mod life_task;
mod mob_ai;
mod mob_brain_plugin;
mod panic_causes;
mod row_hosts;
mod sight_blocks;
mod tile_task;

pub use ai_stats::AiStats;
pub use block_goal_world::BlockGoalWorld;
pub use hot_tick::HotTick;
pub use mob_ai::MobAi;
pub use mob_brain_plugin::MobBrainPlugin;
pub use mob_brain_plugin::MobBrainSet;
pub use panic_causes::PanicCauses;
