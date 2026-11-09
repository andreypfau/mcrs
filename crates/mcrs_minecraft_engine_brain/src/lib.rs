//! The AI stage of the mobs of a dimension world: the host side that publishes the snapshot of
//! the mob rows, applies the decisions through the mob controls and steers the rows before the
//! entity pass, and the stage that runs per tile beside the pass.

mod ai_stats;
mod block_goal_world;
mod cell_table;
mod host_mob;
mod hot_tick;
mod life_task;
mod mob_ai;
mod mob_brain_plugin;
mod order_key;
mod panic_causes;
mod row_hosts;
mod tile_task;

pub use ai_stats::AiStats;
pub use block_goal_world::BlockGoalWorld;
pub use cell_table::CellBox;
pub use cell_table::CellTable;
pub use cell_table::Collision;
pub use cell_table::StateCell;
pub use cell_table::StateFlags;
pub use hot_tick::HotTick;
pub use mcrs_minecraft_physics::PathType;
pub use mob_ai::MobAi;
pub use mob_brain_plugin::GoalCells;
pub use mob_brain_plugin::MobBrainPlugin;
pub use mob_brain_plugin::MobBrainSet;
pub use order_key::order_key;
pub use panic_causes::PanicCauses;
