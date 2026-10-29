use crate::argument_handle::ArgumentHandle;
use crate::build_error::BuildError;
use crate::fork_mode::ForkMode;
use crate::graph::Graph;
use crate::node::Node;
use crate::node_id::NodeId;
use crate::node_kind::NodeKind;
use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;

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

    /// Adds a literal child to `parent`, or returns the equal child it has.
    pub fn literal(
        &mut self,
        parent: NodeId,
        name: &str,
        requirement: R,
    ) -> Result<NodeId, BuildError> {
        self.add(parent, NodeKind::Literal(name.into()), requirement)
    }

    /// Adds an argument child to `parent`, or returns the equal child it has.
    pub fn argument<T>(
        &mut self,
        parent: NodeId,
        name: &str,
        argument: T,
        requirement: R,
    ) -> Result<ArgumentHandle<T>, BuildError>
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
        Ok(ArgumentHandle {
            node,
            argument: PhantomData,
        })
    }

    /// Makes the node executable; a later command replaces an earlier one.
    pub fn executes(&mut self, node: NodeId, command: H) -> Result<(), BuildError> {
        self.node_mut(node)?.command = Some(command);
        Ok(())
    }

    /// Makes a parse continue at the children of `target` after the node.
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
    pub fn modifies(
        &mut self,
        node: NodeId,
        modifier: H,
        mode: ForkMode,
    ) -> Result<(), BuildError> {
        let entry = self.node_mut(node)?;
        entry.modifier = Some(modifier);
        entry.forks = mode == ForkMode::Fork;
        Ok(())
    }

    /// Checks the redirects and the modifiers and returns the tree.
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
