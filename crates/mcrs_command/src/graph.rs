use crate::node::Node;
use crate::node_id::NodeId;
use crate::requirement::Requirement;
use alloc::vec;
use alloc::vec::Vec;

/// A frozen command tree. Node ids are dense, and children keep their registration order.
#[derive(Clone, Debug)]
pub struct Graph<A, R, H> {
    pub(crate) nodes: Vec<Node<A, R, H>>,
}

/// A tree of the root alone.
impl<A, R: Default, H> Default for Graph<A, R, H> {
    fn default() -> Self {
        Self {
            nodes: vec![Node::root()],
        }
    }
}

impl<A, R, H> Graph<A, R, H> {
    /// The node with the id, if the tree has it.
    pub fn node(&self, id: NodeId) -> Option<&Node<A, R, H>> {
        self.nodes.get(id.index())
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Every node with its id, in id order.
    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &Node<A, R, H>)> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (NodeId(index as u32), node))
    }

    /// The node and every node on its path from the root pass for the source.
    pub(crate) fn usable<S>(&self, id: NodeId, source: &S) -> bool
    where
        R: Requirement<S>,
    {
        let mut cursor = Some(id);
        while let Some(current) = cursor.filter(|&id| id != NodeId::ROOT) {
            let Some(node) = self.node(current) else {
                return false;
            };
            if !node.requirement.test(source) {
                return false;
            }
            cursor = node.parent;
        }
        true
    }

    /// The redirect of the node that a parse for the source follows.
    #[must_use]
    pub fn followed_redirect<S>(&self, id: NodeId, source: &S) -> Option<NodeId>
    where
        R: Requirement<S>,
    {
        self.node(id)?
            .redirect
            .filter(|&target| self.usable(target, source))
    }
}
