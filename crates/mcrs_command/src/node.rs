use crate::node_id::NodeId;
use crate::node_kind::NodeKind;
use alloc::vec::Vec;

/// A node of a command tree, over argument set `A`, requirement `R` and handler `H`.
#[derive(Clone, Debug)]
pub struct Node<A, R, H> {
    pub(crate) kind: NodeKind<A>,
    pub(crate) parent: Option<NodeId>,
    pub(crate) children: Vec<NodeId>,
    pub(crate) requirement: R,
    pub(crate) command: Option<H>,
    pub(crate) redirect: Option<NodeId>,
    pub(crate) modifier: Option<H>,
    pub(crate) forks: bool,
}

impl<A, R, H> Node<A, R, H> {
    pub fn kind(&self) -> &NodeKind<A> {
        &self.kind
    }

    /// The argument type of an argument node.
    pub fn argument(&self) -> Option<&A> {
        match &self.kind {
            NodeKind::Argument { argument, .. } => Some(argument),
            _ => None,
        }
    }

    pub(crate) fn is_literal(&self) -> bool {
        matches!(self.kind, NodeKind::Literal(_))
    }

    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    pub fn children(&self) -> &[NodeId] {
        &self.children
    }

    pub fn requirement(&self) -> &R {
        &self.requirement
    }

    pub fn command(&self) -> Option<&H> {
        self.command.as_ref()
    }

    pub fn redirect(&self) -> Option<NodeId> {
        self.redirect
    }

    /// What a redirect does to each source before the target runs; `None` passes the
    /// sources on unchanged.
    pub fn modifier(&self) -> Option<&H> {
        self.modifier.as_ref()
    }

    /// The modifier maps one source to any number of sources, and an error of one source
    /// drops only that source.
    pub fn forks(&self) -> bool {
        self.forks
    }
}

impl<A, R: Default, H> Node<A, R, H> {
    pub(crate) fn root() -> Self {
        Node {
            kind: NodeKind::Root,
            parent: None,
            children: Vec::new(),
            requirement: R::default(),
            command: None,
            redirect: None,
            modifier: None,
            forks: false,
        }
    }
}
