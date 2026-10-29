use crate::node_id::NodeId;
use core::fmt;

/// How a [`GraphBuilder`](crate::GraphBuilder) call failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildError {
    UnknownNode(NodeId),
    /// A child with this name exists with another kind, argument type or requirement.
    Conflict {
        parent: NodeId,
        existing: NodeId,
    },
    /// The node has another redirect already.
    RedirectConflict(NodeId),
    /// A node with a redirect has children.
    RedirectWithChildren(NodeId),
    /// Following the redirects from this node never reaches a node without one.
    RedirectCycle(NodeId),
    /// The node has an argument child with a redirect next to another argument child.
    RedirectSibling(NodeId),
    /// The node has a modifier and no redirect.
    ModifierWithoutRedirect(NodeId),
    TooManyNodes,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::UnknownNode(node) => write!(f, "node {node:?} does not exist"),
            BuildError::Conflict { parent, existing } => write!(
                f,
                "node {parent:?} has a child {existing:?} of that name with other fields"
            ),
            BuildError::RedirectConflict(node) => {
                write!(f, "node {node:?} already has another redirect")
            }
            BuildError::RedirectWithChildren(node) => {
                write!(f, "node {node:?} has both a redirect and children")
            }
            BuildError::RedirectCycle(node) => {
                write!(f, "the redirects from node {node:?} form a cycle")
            }
            BuildError::RedirectSibling(node) => write!(
                f,
                "node {node:?} has an argument child with a redirect and another argument child"
            ),
            BuildError::ModifierWithoutRedirect(node) => {
                write!(f, "node {node:?} has a modifier and no redirect")
            }
            BuildError::TooManyNodes => f.write_str("the tree has more nodes than an id can hold"),
        }
    }
}

impl core::error::Error for BuildError {}
