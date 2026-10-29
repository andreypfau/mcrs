/// The id of a node, dense from the root at 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub(crate) u32);

impl NodeId {
    pub const ROOT: NodeId = NodeId(0);

    /// The position of the node in [`Graph`](crate::Graph).
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}
