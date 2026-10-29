use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;

/// A predicate over the source of a command, held as data so the host can also inspect it.
pub trait Requirement<S> {
    fn test(&self, source: &S) -> bool;
}

/// No requirement: every source passes.
impl<S> Requirement<S> for () {
    fn test(&self, _: &S) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u32);

impl NodeId {
    pub const ROOT: NodeId = NodeId(0);

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// A typed handle to an argument node; the handler reads the value through it.
pub struct Arg<T> {
    node: NodeId,
    argument: PhantomData<fn() -> T>,
}

impl<T> Arg<T> {
    pub const fn node(self) -> NodeId {
        self.node
    }
}

/// Whether the node holds a `T` is checked when the value is read.
impl<T> From<NodeId> for Arg<T> {
    fn from(node: NodeId) -> Self {
        Self {
            node,
            argument: PhantomData,
        }
    }
}

impl<T> From<Arg<T>> for NodeId {
    fn from(arg: Arg<T>) -> Self {
        arg.node
    }
}

impl<T> Clone for Arg<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Arg<T> {}

impl<T> core::fmt::Debug for Arg<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Arg").field(&self.node).finish()
    }
}

/// An argument set that holds the argument type `T` as one of its variants.
pub trait Contains<T> {
    fn get(&self) -> Option<&T>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind<A> {
    Root,
    Literal(Box<str>),
    Argument { name: Box<str>, argument: A },
}

impl<A> NodeKind<A> {
    pub fn name(&self) -> &str {
        match self {
            NodeKind::Root => "",
            NodeKind::Literal(name) | NodeKind::Argument { name, .. } => name,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Node<A, R, H> {
    kind: NodeKind<A>,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    requirement: R,
    command: Option<H>,
    redirect: Option<NodeId>,
    modifier: Option<H>,
    forks: bool,
}

impl<A, R, H> Node<A, R, H> {
    pub fn kind(&self) -> &NodeKind<A> {
        &self.kind
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

/// A command tree under construction. A node added twice under one parent merges.
#[derive(Clone, Debug)]
pub struct GraphBuilder<A, R, H> {
    nodes: Vec<Node<A, R, H>>,
}

impl<A: PartialEq, R: PartialEq + Default, H> Default for GraphBuilder<A, R, H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: PartialEq, R: PartialEq + Default, H> GraphBuilder<A, R, H> {
    pub fn new() -> Self {
        Self {
            nodes: vec![Node::root()],
        }
    }

    pub fn literal(
        &mut self,
        parent: NodeId,
        name: &str,
        requirement: R,
    ) -> Result<NodeId, BuildError> {
        self.add(parent, NodeKind::Literal(name.into()), requirement)
    }

    pub fn argument<T>(
        &mut self,
        parent: NodeId,
        name: &str,
        argument: T,
        requirement: R,
    ) -> Result<Arg<T>, BuildError>
    where
        T: Into<A>,
    {
        let node = self.add(
            parent,
            NodeKind::Argument {
                name: name.into(),
                argument: argument.into(),
            },
            requirement,
        )?;
        Ok(Arg {
            node,
            argument: PhantomData,
        })
    }

    /// Makes the node executable; a later command replaces an earlier one.
    pub fn executes(&mut self, node: NodeId, command: H) -> Result<(), BuildError> {
        self.node_mut(node)?.command = Some(command);
        Ok(())
    }

    pub fn redirect(&mut self, node: NodeId, target: NodeId) -> Result<(), BuildError> {
        if self.nodes.get(target.index()).is_none() {
            return Err(BuildError::UnknownNode(target));
        }
        let entry = self.node_mut(node)?;
        if !entry.children.is_empty() {
            return Err(BuildError::RedirectWithChildren(node));
        }
        match &entry.redirect {
            Some(existing) if *existing != target => Err(BuildError::RedirectConflict(node)),
            _ => {
                entry.redirect = Some(target);
                Ok(())
            }
        }
    }

    /// Gives the redirect of the node a modifier. A node with a modifier must have a redirect
    /// when the tree freezes.
    pub fn modifies(&mut self, node: NodeId, modifier: H, forks: bool) -> Result<(), BuildError> {
        let entry = self.node_mut(node)?;
        entry.modifier = Some(modifier);
        entry.forks = forks;
        Ok(())
    }

    pub fn freeze(self) -> Result<Graph<A, R, H>, BuildError> {
        for (index, node) in self.nodes.iter().enumerate() {
            let id = NodeId(index as u32);
            if node.modifier.is_some() && node.redirect.is_none() {
                return Err(BuildError::ModifierWithoutRedirect(id));
            }
            let mut hops = 0;
            let mut cursor = node;
            while let Some(target) = cursor.redirect {
                hops += 1;
                if hops > self.nodes.len() {
                    return Err(BuildError::RedirectCycle(id));
                }
                cursor = self
                    .nodes
                    .get(target.index())
                    .ok_or(BuildError::UnknownNode(target))?;
            }
            let arguments = || {
                node.children.iter().filter_map(|child| {
                    self.nodes
                        .get(child.index())
                        .filter(|child| matches!(child.kind, NodeKind::Argument { .. }))
                })
            };
            if arguments().count() > 1 && arguments().any(|child| child.redirect.is_some()) {
                return Err(BuildError::RedirectSibling(id));
            }
        }
        Ok(Graph { nodes: self.nodes })
    }

    /// Makes an existing node a child of one more parent. The node keeps its first parent, so
    /// its requirement chain is the path through that parent.
    pub fn link(&mut self, parent: NodeId, child: NodeId) -> Result<(), BuildError> {
        let child_node = self
            .nodes
            .get(child.index())
            .ok_or(BuildError::UnknownNode(child))?;
        if let Some((existing, _)) = self.child_named(parent, child_node.kind.name())? {
            return if existing == child {
                Ok(())
            } else {
                Err(BuildError::Conflict { parent, existing })
            };
        }
        self.node_mut(parent)?.children.push(child);
        Ok(())
    }

    /// The child of `parent` with `name`. A parent that redirects takes no child.
    fn child_named(
        &self,
        parent: NodeId,
        name: &str,
    ) -> Result<Option<(NodeId, &Node<A, R, H>)>, BuildError> {
        let parent_node = self
            .nodes
            .get(parent.index())
            .ok_or(BuildError::UnknownNode(parent))?;
        if parent_node.redirect.is_some() {
            return Err(BuildError::RedirectWithChildren(parent));
        }
        Ok(parent_node.children.iter().find_map(|&child| {
            let node = self.nodes.get(child.index())?;
            (node.kind.name() == name).then_some((child, node))
        }))
    }

    fn node_mut(&mut self, node: NodeId) -> Result<&mut Node<A, R, H>, BuildError> {
        self.nodes
            .get_mut(node.index())
            .ok_or(BuildError::UnknownNode(node))
    }

    fn add(
        &mut self,
        parent: NodeId,
        kind: NodeKind<A>,
        requirement: R,
    ) -> Result<NodeId, BuildError> {
        let existing = self.child_named(parent, kind.name())?;
        if let Some((existing, node)) = existing {
            return if node.kind == kind && node.requirement == requirement {
                Ok(existing)
            } else {
                Err(BuildError::Conflict { parent, existing })
            };
        }
        let id = NodeId(u32::try_from(self.nodes.len()).map_err(|_| BuildError::TooManyNodes)?);
        self.nodes.push(Node {
            kind,
            parent: Some(parent),
            children: Vec::new(),
            requirement,
            command: None,
            redirect: None,
            modifier: None,
            forks: false,
        });
        self.node_mut(parent)?.children.push(id);
        Ok(id)
    }
}

/// A frozen command tree. Node ids are dense, and children keep their registration order.
#[derive(Clone, Debug)]
pub struct Graph<A, R, H> {
    nodes: Vec<Node<A, R, H>>,
}

impl<A, R: Default, H> Node<A, R, H> {
    fn root() -> Self {
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

/// A tree of the root alone.
impl<A, R: Default, H> Default for Graph<A, R, H> {
    fn default() -> Self {
        Self {
            nodes: vec![Node::root()],
        }
    }
}

impl<A, R, H> Graph<A, R, H> {
    pub fn node(&self, id: NodeId) -> Option<&Node<A, R, H>> {
        self.nodes.get(id.index())
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

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
    pub fn followed_redirect<S>(&self, id: NodeId, source: &S) -> Option<NodeId>
    where
        R: Requirement<S>,
    {
        self.node(id)?
            .redirect
            .filter(|&target| self.usable(target, source))
    }
}
