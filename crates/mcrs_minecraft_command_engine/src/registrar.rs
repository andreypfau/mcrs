use crate::handlers::HandlerKey;
use mcrs_command::{ArgumentHandle, BuildError, ForkMode, GraphBuilder, NodeId};
use mcrs_minecraft_command::PermissionCheck;
use mcrs_minecraft_command_vanilla::VanillaArgument;
use std::marker::PhantomData;

/// The command an executable node runs, and which branch of that command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Handler {
    pub(crate) key: HandlerKey,
    pub(crate) branch: u16,
}

pub(crate) type Builder = GraphBuilder<VanillaArgument, PermissionCheck, Handler>;

/// Registers the nodes of one command whose branches are the values of `B`.
pub struct CommandRegistrar<'a, B> {
    pub(crate) builder: &'a mut Builder,
    pub(crate) key: HandlerKey,
    pub(crate) branches: Vec<B>,
}

impl<'a, B: Copy + PartialEq> CommandRegistrar<'a, B> {
    /// A literal node named `name` under `parent`.
    pub fn literal<'r>(
        &'r mut self,
        parent: impl Into<NodeId>,
        name: &'r str,
    ) -> NodeRef<'r, 'a, B, NodeId> {
        NodeRef::new(self, parent.into(), name, None)
    }

    /// An argument node named `name` under `parent`, which parses `argument`.
    pub fn arg<'r, T: Into<VanillaArgument>>(
        &'r mut self,
        parent: impl Into<NodeId>,
        name: &'r str,
        argument: T,
    ) -> NodeRef<'r, 'a, B, ArgumentHandle<T>> {
        NodeRef::new(self, parent.into(), name, Some(argument.into()))
    }

    fn branch(&mut self, branch: B) -> u16 {
        let index = match self.branches.iter().position(|known| *known == branch) {
            Some(index) => index,
            None => {
                self.branches.push(branch);
                self.branches.len() - 1
            }
        };
        u16::try_from(index).unwrap_or(u16::MAX)
    }
}

/// The modifier of a redirect: the branch that maps a source, and how many sources it gives.
#[derive(Clone, Copy)]
struct Modifier<B> {
    branch: B,
    mode: ForkMode,
}

/// A node that [`NodeRef::id`] adds to the tree.
#[must_use]
pub struct NodeRef<'r, 'a, B, K> {
    registrar: &'r mut CommandRegistrar<'a, B>,
    parent: NodeId,
    name: &'r str,
    argument: Option<VanillaArgument>,
    check: PermissionCheck,
    branch: Option<B>,
    redirect: Option<NodeId>,
    modifier: Option<Modifier<B>>,
    handle: PhantomData<fn() -> K>,
}

impl<'r, 'a, B: Copy + PartialEq, K: From<NodeId>> NodeRef<'r, 'a, B, K> {
    fn new(
        registrar: &'r mut CommandRegistrar<'a, B>,
        parent: NodeId,
        name: &'r str,
        argument: Option<VanillaArgument>,
    ) -> Self {
        Self {
            registrar,
            parent,
            name,
            argument,
            check: PermissionCheck::AlwaysPass,
            branch: None,
            redirect: None,
            modifier: None,
            handle: PhantomData,
        }
    }

    /// The permission a source needs to see and use the node.
    pub fn requires(mut self, check: PermissionCheck) -> Self {
        self.check = check;
        self
    }

    /// Makes the node executable: it runs `branch` of the command.
    pub fn runs(mut self, branch: B) -> Self {
        self.branch = Some(branch);
        self
    }

    /// Continues the parse at `target` with the same source.
    pub fn redirect(mut self, target: impl Into<NodeId>) -> Self {
        self.redirect = Some(target.into());
        self
    }

    /// A redirect whose modifier maps each source to one source, or fails.
    pub fn modify(mut self, target: impl Into<NodeId>, branch: B) -> Self {
        self.redirect = Some(target.into());
        self.modifier = Some(Modifier {
            branch,
            mode: ForkMode::Single,
        });
        self
    }

    /// A redirect whose modifier maps each source to any number of sources.
    pub fn fork(mut self, target: impl Into<NodeId>, branch: B) -> Self {
        self.redirect = Some(target.into());
        self.modifier = Some(Modifier {
            branch,
            mode: ForkMode::Fork,
        });
        self
    }

    /// Adds the node to the tree and gives its handle.
    pub fn id(self) -> Result<K, BuildError> {
        let registrar = self.registrar;
        let builder = &mut *registrar.builder;
        let node = match self.argument {
            Some(argument) => builder
                .argument(self.parent, self.name, argument, self.check)
                .map(ArgumentHandle::node),
            None => builder.literal(self.parent, self.name, self.check),
        }?;
        if let Some(branch) = self.branch {
            let handler = Handler {
                key: registrar.key,
                branch: registrar.branch(branch),
            };
            registrar.builder.executes(node, handler)?;
        }
        if let Some(target) = self.redirect {
            registrar.builder.redirect(node, target)?;
        }
        if let Some(Modifier { branch, mode }) = self.modifier {
            let handler = Handler {
                key: registrar.key,
                branch: registrar.branch(branch),
            };
            registrar.builder.modifies(node, handler, mode)?;
        }
        Ok(K::from(node))
    }
}
