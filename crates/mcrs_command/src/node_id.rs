#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub(crate) u32);

impl NodeId {
    pub const ROOT: NodeId = NodeId(0);

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}
