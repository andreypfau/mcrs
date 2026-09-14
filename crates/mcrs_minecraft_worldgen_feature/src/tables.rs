use rustc_hash::FxHashMap as HashMap;

use mcrs_minecraft_chunk::VoxelId;

use crate::place::huge_mushroom::MushroomFaces;
use crate::place::mossy_carpet::MossyCarpetStates;
use crate::place::patch::Waterlogging;
use crate::place::simple_block::DoublePlant;

/// Every state table a feature reads and none configures: a function of the
/// block definitions alone, so one dimension needs exactly one of these.
///
/// Resolved beside [`WorldStates`](crate::placer::WorldStates)
/// and shared from there. A table built per configured feature instead would be
/// the same bytes once per entry of the corpus, and `simple_block` alone has
/// seventy-five of those.
#[derive(Clone, Debug, Default)]
pub struct BlockTables {
    /// Keyed by the lower half, which is the state a provider hands back.
    pub double_plants: HashMap<u16, DoublePlant>,
    /// `None` where the corpus holds no `pale_moss_carpet`.
    pub mossy_carpet: Option<MossyCarpetStates>,
    /// Every state carrying `snowy`, mapped to the same state with it set.
    pub snowy: HashMap<VoxelId, VoxelId>,
    pub waterlogging: Waterlogging,
    /// A vine attached on one face, indexed over `Direction::all()`; the down
    /// slot is never read, since a vine has no bottom face. `None` where the
    /// corpus holds no vine.
    pub vine: Option<[VoxelId; 6]>,
    pub mushroom_faces: MushroomFaces,
}
