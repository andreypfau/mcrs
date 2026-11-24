use crate::{Node, NodeId, PathScratch};
use mcrs_minecraft_physics::EntityCollider;
use mcrs_voxel::BlockPos;
use rustc_hash::FxHashMap;

pub struct SearchScratch<S> {
    pub(crate) nodes: Vec<Node>,
    pub(crate) positions: FxHashMap<BlockPos, NodeId>,
    pub(crate) paths: PathScratch<S>,
    pub(crate) collisions: FxHashMap<[u64; 6], bool>,
    pub(crate) entities: Vec<EntityCollider>,
}
impl<S> Default for SearchScratch<S> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            positions: FxHashMap::default(),
            paths: PathScratch::default(),
            collisions: FxHashMap::default(),
            entities: Vec::new(),
        }
    }
}
impl<S> SearchScratch<S> {
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.positions.clear();
        self.paths.clear();
        self.collisions.clear();
        self.entities.clear();
    }
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0)
    }
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id.0)
    }
    pub(crate) fn at(&mut self, position: BlockPos) -> NodeId {
        *self.positions.entry(position).or_insert_with(|| {
            let id = NodeId(self.nodes.len());
            self.nodes.push(Node::new(position));
            id
        })
    }
}
