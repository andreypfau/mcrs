use mcrs_minecraft_physics::PathType;
use mcrs_voxel::BlockPos;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) usize);

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Node {
    pub position: BlockPos,
    pub g: f32,
    pub h: f32,
    pub f: f32,
    pub walked: f32,
    pub kind: PathType,
    pub malus: f32,
    pub previous: Option<NodeId>,
    pub closed: bool,
    pub heap_index: Option<usize>,
}
impl Node {
    #[must_use]
    pub(crate) fn new(position: BlockPos) -> Self {
        Self {
            position,
            g: 0.0,
            h: 0.0,
            f: 0.0,
            walked: 0.0,
            kind: PathType::Blocked,
            malus: 0.0,
            previous: None,
            closed: false,
            heap_index: None,
        }
    }
}
