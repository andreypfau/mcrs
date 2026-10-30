use crate::command_set::CommandSet;
use crate::handlers::HandlerKey;
use mcrs_command::{ArgumentHandle, BuildError, ForkMode, GraphBuilder, NodeId};
use mcrs_minecraft_command::PermissionCheck;
use std::marker::PhantomData;

/// The command an executable node runs, and which branch of that command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Handler {
    pub(crate) key: HandlerKey,
    pub(crate) branch: u16,
}

pub(crate) type Builder<S> = GraphBuilder<<S as CommandSet>::Argument, PermissionCheck, Handler>;

/// The handler tables the nodes of one branch use.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TablesUsed {
    Run,
    Modify,
    Both,
}

impl TablesUsed {
    pub(crate) fn runs(self) -> bool {
        matches!(self, Self::Run | Self::Both)
    }

    pub(crate) fn modifies(self) -> bool {
        matches!(self, Self::Modify | Self::Both)
    }

    fn with(self, table: Table) -> Self {
        if self == Self::from(table) {
            self
        } else {
            Self::Both
        }
    }
}

impl From<Table> for TablesUsed {
    fn from(table: Table) -> Self {
        match table {
            Table::Run => Self::Run,
            Table::Modify => Self::Modify,
        }
    }
}

/// A branch of a command and which of the two handler tables its nodes use.
#[derive(Clone, Copy)]
pub(crate) struct BranchUse<B> {
    pub(crate) branch: B,
    pub(crate) tables: TablesUsed,
}

/// Registers the nodes of one command of the set `S` whose branches are the values of `B`.
pub struct CommandRegistrar<'a, S: CommandSet, B> {
    pub(crate) builder: &'a mut Builder<S>,
    pub(crate) key: HandlerKey,
    pub(crate) branches: Vec<BranchUse<B>>,
    pub(crate) overflowed: bool,
}

impl<'a, S: CommandSet, B: Copy + PartialEq> CommandRegistrar<'a, S, B> {
    /// A literal node named `name` under `parent`.
    pub fn literal<'r>(
        &'r mut self,
        parent: impl Into<NodeId>,
        name: &'r str,
    ) -> NodeRef<'r, 'a, S, B, NodeId> {
        NodeRef::new(self, parent.into(), name, None)
    }

    /// An argument node named `name` under `parent`, which parses `argument`.
    pub fn arg<'r, T: Into<S::Argument>>(
        &'r mut self,
        parent: impl Into<NodeId>,
        name: &'r str,
        argument: T,
    ) -> NodeRef<'r, 'a, S, B, ArgumentHandle<T>> {
        NodeRef::new(self, parent.into(), name, Some(argument.into()))
    }

    fn handler(&mut self, branch: B, table: Table) -> Option<Handler> {
        let index = match self
            .branches
            .iter()
            .position(|known| known.branch == branch)
        {
            Some(index) => index,
            None => {
                self.branches.push(BranchUse {
                    branch,
                    tables: table.into(),
                });
                self.branches.len() - 1
            }
        };
        let Ok(slot) = u16::try_from(index) else {
            self.overflowed = true;
            return None;
        };
        let used = self.branches.get_mut(index)?;
        used.tables = used.tables.with(table);
        Some(Handler {
            key: self.key,
            branch: slot,
        })
    }
}

/// One of the two handler tables of a command.
#[derive(Clone, Copy)]
enum Table {
    Run,
    Modify,
}

/// The modifier of a redirect: the branch that maps a source, and how many sources it gives.
#[derive(Clone, Copy)]
struct Modifier<B> {
    branch: B,
    mode: ForkMode,
}

/// A node that [`NodeRef::id`] adds to the tree.
#[must_use]
pub struct NodeRef<'r, 'a, S: CommandSet, B, K> {
    registrar: &'r mut CommandRegistrar<'a, S, B>,
    parent: NodeId,
    name: &'r str,
    argument: Option<S::Argument>,
    check: PermissionCheck,
    branch: Option<B>,
    redirect: Option<NodeId>,
    modifier: Option<Modifier<B>>,
    handle: PhantomData<fn() -> K>,
}

impl<'r, 'a, S: CommandSet, B: Copy + PartialEq, K: From<NodeId>> NodeRef<'r, 'a, S, B, K> {
    fn new(
        registrar: &'r mut CommandRegistrar<'a, S, B>,
        parent: NodeId,
        name: &'r str,
        argument: Option<S::Argument>,
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
            let handler = registrar
                .handler(branch, Table::Run)
                .ok_or(BuildError::TooManyNodes)?;
            registrar.builder.executes(node, handler)?;
        }
        if let Some(target) = self.redirect {
            registrar.builder.redirect(node, target)?;
        }
        if let Some(Modifier { branch, mode }) = self.modifier {
            let handler = registrar
                .handler(branch, Table::Modify)
                .ok_or(BuildError::TooManyNodes)?;
            registrar.builder.modifies(node, handler, mode)?;
        }
        Ok(K::from(node))
    }
}
