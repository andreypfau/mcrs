use crate::command_set::CommandSet;
use crate::handlers::HandlerKey;
use mcrs_command::{ArgumentHandle, BuildError, ForkMode, GraphBuilder, NodeId};
use mcrs_minecraft_command::PermissionCheck;
use std::marker::PhantomData;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Handler {
    pub(crate) key: HandlerKey,
    pub(crate) branch: u16,
    pub(crate) world: HandlerWorld,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HandlerWorld {
    SourceDimension,
    Any,
}

pub(crate) type Builder<S> = GraphBuilder<<S as CommandSet>::Argument, PermissionCheck, Handler>;

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

#[derive(Clone, Copy)]
pub(crate) struct BranchUse<B> {
    pub(crate) branch: B,
    pub(crate) tables: TablesUsed,
}

pub struct CommandRegistrar<'a, S: CommandSet, B> {
    pub(crate) builder: &'a mut Builder<S>,
    pub(crate) key: HandlerKey,
    pub(crate) branches: Vec<BranchUse<B>>,
    pub(crate) overflowed: bool,
}

impl<'a, S: CommandSet, B: Copy + PartialEq> CommandRegistrar<'a, S, B> {
    pub fn literal<'r>(
        &'r mut self,
        parent: impl Into<NodeId>,
        name: &'r str,
    ) -> NodeRef<'r, 'a, S, B, NodeId> {
        NodeRef::new(self, parent.into(), name, None)
    }

    pub fn arg<'r, T: Into<S::Argument>>(
        &'r mut self,
        parent: impl Into<NodeId>,
        name: &'r str,
        argument: T,
    ) -> NodeRef<'r, 'a, S, B, ArgumentHandle<T>> {
        NodeRef::new(self, parent.into(), name, Some(argument.into()))
    }

    fn handler(&mut self, branch: B, table: Table, world: HandlerWorld) -> Option<Handler> {
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
            world,
        })
    }
}

#[derive(Clone, Copy)]
enum Table {
    Run,
    Modify,
}

#[derive(Clone, Copy)]
struct Modifier<B> {
    branch: B,
    mode: ForkMode,
}

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
    world: HandlerWorld,
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
            world: HandlerWorld::SourceDimension,
            handle: PhantomData,
        }
    }

    pub fn requires(mut self, check: PermissionCheck) -> Self {
        self.check = check;
        self
    }

    pub fn runs(mut self, branch: B) -> Self {
        self.branch = Some(branch);
        self
    }

    pub fn redirect(mut self, target: impl Into<NodeId>) -> Self {
        self.redirect = Some(target.into());
        self
    }

    pub fn modify(mut self, target: impl Into<NodeId>, branch: B) -> Self {
        self.redirect = Some(target.into());
        self.modifier = Some(Modifier {
            branch,
            mode: ForkMode::Single,
        });
        self
    }

    pub fn fork(mut self, target: impl Into<NodeId>, branch: B) -> Self {
        self.redirect = Some(target.into());
        self.modifier = Some(Modifier {
            branch,
            mode: ForkMode::Fork,
        });
        self
    }

    pub fn in_any_world(mut self) -> Self {
        self.world = HandlerWorld::Any;
        self
    }

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
                .handler(branch, Table::Run, self.world)
                .ok_or(BuildError::TooManyNodes)?;
            registrar.builder.executes(node, handler)?;
        }
        if let Some(target) = self.redirect {
            registrar.builder.redirect(node, target)?;
        }
        if let Some(Modifier { branch, mode }) = self.modifier {
            let handler = registrar
                .handler(branch, Table::Modify, self.world)
                .ok_or(BuildError::TooManyNodes)?;
            registrar.builder.modifies(node, handler, mode)?;
        }
        Ok(K::from(node))
    }
}
