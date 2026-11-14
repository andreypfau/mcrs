//! The rules of a falling block without Bevy: which blocks fall, the start of a fall, the
//! decision of each tick and the landing.
// std: mcrs_minecraft_block

mod after_move;
mod block_tag_keys;
mod concrete_pairs;
mod concrete_pairs_error;
mod concrete_table;
mod falling_block;
mod falling_block_table;
mod landing;
mod remains;
mod settings;
mod start;
mod state_flags;
mod water_contact;

pub use after_move::AfterMove;
pub use after_move::Ground;
pub use after_move::WorldHeight;
pub use block_tag_keys::BREAKS_AFTER_FALL;
pub use block_tag_keys::GRAVITY_BLOCKS;
pub use concrete_pairs::CONCRETE_PAIRS_PATH;
pub use concrete_pairs::ConcretePairsFile;
pub use concrete_pairs::PackPairs;
pub use concrete_pairs_error::ConcretePairsError;
pub use concrete_pairs_error::ConcretePairsErrorKind;
pub use concrete_table::ConcreteTable;
pub use falling_block::AtLanding;
pub use falling_block::FallingBlock;
pub use falling_block::ItemDrop;
pub use falling_block::TickStart;
pub use falling_block_table::FallingBlockTable;
pub use landing::Landing;
pub use landing::LandingOutcome;
pub use landing::LandingSite;
pub use landing::LevelEvent;
pub use landing::Placement;
pub use landing::Replaceable;
pub use landing::Silence;
pub use landing::Support;
pub use landing::WaterBeside;
pub use remains::EntityDrops;
pub use remains::Remains;
pub use settings::Settings;
pub use start::Start;
pub use start::start;
pub use water_contact::WaterContact;
pub use water_contact::WaterSweep;
