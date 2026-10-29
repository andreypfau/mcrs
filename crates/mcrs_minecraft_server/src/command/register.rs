use crate::command::effect::EffectPlugin;
use crate::command::{
    CommandResult, CommandTreeBuilder, HandlerKey, Handlers, Invocation, ModifyResult, Worlds,
};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, ForkMode, GraphBuilder, NodeId};
use mcrs_minecraft_command::{PermissionCheck, VanillaArgument};
use mcrs_minecraft_level::world::sub_app::AddDimensionPlugins;
use std::marker::PhantomData;
use std::ops::Deref;

/// The command an executable node runs, and which branch of that command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Handler {
    pub(crate) key: HandlerKey,
    pub(crate) branch: u16,
}

type Builder = GraphBuilder<VanillaArgument, PermissionCheck, Handler>;

/// Registers the nodes of one command whose branches are the values of `B`.
pub struct CommandRegistrar<'a, B> {
    builder: &'a mut Builder,
    key: HandlerKey,
    branches: Vec<B>,
}

impl<'a, B: Copy + PartialEq> CommandRegistrar<'a, B> {
    pub fn literal<'r>(
        &'r mut self,
        parent: impl Into<NodeId>,
        name: &'r str,
    ) -> NodeRef<'r, 'a, B, NodeId> {
        NodeRef::new(self, parent.into(), name, None)
    }

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
    modifier: Option<(B, ForkMode)>,
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

    /// A redirect whose modifier maps each source to one source, or fails.
    pub fn modify(mut self, target: impl Into<NodeId>, branch: B) -> Self {
        self.redirect = Some(target.into());
        self.modifier = Some((branch, ForkMode::Single));
        self
    }

    /// A redirect whose modifier maps each source to any number of sources.
    pub fn fork(mut self, target: impl Into<NodeId>, branch: B) -> Self {
        self.redirect = Some(target.into());
        self.modifier = Some((branch, ForkMode::Fork));
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
            let handler = Handler {
                key: registrar.key,
                branch: registrar.branch(branch),
            };
            registrar.builder.executes(node, handler)?;
        }
        if let Some(target) = self.redirect {
            registrar.builder.redirect(node, target)?;
        }
        if let Some((branch, mode)) = self.modifier {
            let handler = Handler {
                key: registrar.key,
                branch: registrar.branch(branch),
            };
            registrar.builder.modifies(node, handler, mode)?;
        }
        Ok(K::from(node))
    }
}

/// The command line a handler runs, the branch its executable node names, and the handles
/// the command's builder returned.
#[derive(Clone)]
pub struct Call<B, C> {
    pub branch: B,
    pub command: C,
    invocation: Invocation,
}

impl<B, C> Deref for Call<B, C> {
    type Target = Invocation;

    fn deref(&self) -> &Invocation {
        &self.invocation
    }
}

#[derive(Clone)]
struct BranchTable<B> {
    first: B,
    rest: Vec<B>,
}

impl<B: Copy> BranchTable<B> {
    /// Every index a node holds came from this table, so the fallback is never taken.
    fn get(&self, index: u16) -> B {
        match index.checked_sub(1) {
            None => self.first,
            Some(rest) => self
                .rest
                .get(usize::from(rest))
                .copied()
                .unwrap_or(self.first),
        }
    }
}

/// Each registration owns its table and handles, so commands that share `B` or `C` stay apart.
fn to_call<B: Copy, C: Copy>(
    table: BranchTable<B>,
    command: C,
) -> impl FnMut(In<Invocation>) -> Call<B, C> + Clone {
    move |In(invocation)| Call {
        branch: table.get(invocation.branch),
        command,
        invocation,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RegistrationError {
    #[error("the command tree does not take the command: {0}")]
    Tree(#[from] BuildError),
    #[error("commands are added after CommandPlugin and before the app finishes")]
    NoTree,
    #[error("the command tree holds as many handlers as a handler key can count")]
    TooManyCommands,
}

pub trait AppCommands {
    /// Registers a command: `build` adds its nodes and returns the handles its handler reads,
    /// and every dimension world runs `handler` when one of its branches runs. Nothing is
    /// registered when `build` returns an error.
    fn add_command<B, C, S, M>(
        &mut self,
        build: impl FnOnce(&mut CommandRegistrar<'_, B>) -> Result<C, BuildError>,
        handler: S,
    ) -> Result<&mut Self, RegistrationError>
    where
        B: Copy + PartialEq + Send + Sync + 'static,
        C: Copy + Send + Sync + 'static,
        S: IntoSystem<In<Call<B, C>>, CommandResult, M> + Clone + Send + Sync + 'static,
        M: 'static;

    /// Registers a command whose redirects have modifiers: every dimension world runs
    /// `modify` for a source when a stage of a chain follows one of them.
    fn add_modifying_command<B, C, S, M, D, N>(
        &mut self,
        build: impl FnOnce(&mut CommandRegistrar<'_, B>) -> Result<C, BuildError>,
        handler: S,
        modify: D,
    ) -> Result<&mut Self, RegistrationError>
    where
        B: Copy + PartialEq + Send + Sync + 'static,
        C: Copy + Send + Sync + 'static,
        S: IntoSystem<In<Call<B, C>>, CommandResult, M> + Clone + Send + Sync + 'static,
        M: 'static,
        D: IntoSystem<In<Call<B, C>>, ModifyResult, N> + Clone + Send + Sync + 'static,
        N: 'static;

    /// Registers a command effect type: every dimension world gets an
    /// [`EffectQueue<E>`](crate::command::EffectQueue), and the host applies each effect that a
    /// handler writes with `apply`, in the order of writing across all effect types. A later
    /// registration of the same type replaces `apply`.
    fn add_command_effect<E: Send + Sync + 'static>(
        &mut self,
        apply: fn(&mut Worlds<'_, '_>, E),
    ) -> &mut Self;
}

/// Adds the nodes of one command to the tree, or nothing when `build` returns an error.
fn register<B, C>(
    app: &mut App,
    build: impl FnOnce(&mut CommandRegistrar<'_, B>) -> Result<C, BuildError>,
) -> Result<Option<(C, HandlerKey, BranchTable<B>)>, RegistrationError>
where
    B: Copy + PartialEq,
{
    let mut tree = app
        .world_mut()
        .get_resource_mut::<CommandTreeBuilder>()
        .ok_or(RegistrationError::NoTree)?;
    let tree = &mut *tree;
    let key = HandlerKey(tree.next);
    let next = tree
        .next
        .checked_add(1)
        .ok_or(RegistrationError::TooManyCommands)?;
    let before = tree.builder.clone();
    let mut registrar = CommandRegistrar {
        builder: &mut tree.builder,
        key,
        branches: Vec::new(),
    };
    let handles = match build(&mut registrar) {
        Ok(handles) => handles,
        Err(error) => {
            tree.builder = before;
            return Err(error.into());
        }
    };
    let branches = registrar.branches;
    tree.next = next;
    let Some((&first, rest)) = branches.split_first() else {
        return Ok(None);
    };
    let table = BranchTable {
        first,
        rest: rest.to_vec(),
    };
    Ok(Some((handles, key, table)))
}

impl AppCommands for App {
    fn add_command<B, C, S, M>(
        &mut self,
        build: impl FnOnce(&mut CommandRegistrar<'_, B>) -> Result<C, BuildError>,
        handler: S,
    ) -> Result<&mut Self, RegistrationError>
    where
        B: Copy + PartialEq + Send + Sync + 'static,
        C: Copy + Send + Sync + 'static,
        S: IntoSystem<In<Call<B, C>>, CommandResult, M> + Clone + Send + Sync + 'static,
        M: 'static,
    {
        let Some((handles, key, table)) = register(self, build)? else {
            return Ok(self);
        };
        Ok(self.add_dimension_plugin(HandlerPlugin {
            handles,
            key,
            table,
            system: handler,
            marker: PhantomData::<fn() -> (M, CommandResult)>,
        }))
    }

    fn add_modifying_command<B, C, S, M, D, N>(
        &mut self,
        build: impl FnOnce(&mut CommandRegistrar<'_, B>) -> Result<C, BuildError>,
        handler: S,
        modify: D,
    ) -> Result<&mut Self, RegistrationError>
    where
        B: Copy + PartialEq + Send + Sync + 'static,
        C: Copy + Send + Sync + 'static,
        S: IntoSystem<In<Call<B, C>>, CommandResult, M> + Clone + Send + Sync + 'static,
        M: 'static,
        D: IntoSystem<In<Call<B, C>>, ModifyResult, N> + Clone + Send + Sync + 'static,
        N: 'static,
    {
        let Some((handles, key, table)) = register(self, build)? else {
            return Ok(self);
        };
        self.add_dimension_plugin(HandlerPlugin {
            handles,
            key,
            table: table.clone(),
            system: handler,
            marker: PhantomData::<fn() -> (M, CommandResult)>,
        });
        Ok(self.add_dimension_plugin(HandlerPlugin {
            handles,
            key,
            table,
            system: modify,
            marker: PhantomData::<fn() -> (N, ModifyResult)>,
        }))
    }

    fn add_command_effect<E: Send + Sync + 'static>(
        &mut self,
        apply: fn(&mut Worlds<'_, '_>, E),
    ) -> &mut Self {
        self.add_dimension_plugin(EffectPlugin { apply })
    }
}

/// Registers the handler of a command in each dimension, under its key, in the table of its
/// output: a [`CommandResult`] or a [`ModifyResult`].
struct HandlerPlugin<B, C, S, M> {
    handles: C,
    key: HandlerKey,
    table: BranchTable<B>,
    system: S,
    marker: PhantomData<fn() -> M>,
}

impl<B: Clone, C: Clone, S: Clone, M> Clone for HandlerPlugin<B, C, S, M> {
    fn clone(&self) -> Self {
        Self {
            handles: self.handles.clone(),
            key: self.key,
            table: self.table.clone(),
            system: self.system.clone(),
            marker: PhantomData,
        }
    }
}

impl<B, C, S, M, O> Plugin for HandlerPlugin<B, C, S, (M, O)>
where
    B: Copy + PartialEq + Send + Sync + 'static,
    C: Copy + Send + Sync + 'static,
    S: IntoSystem<In<Call<B, C>>, O, M> + Clone + Send + Sync + 'static,
    M: 'static,
    O: Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        let id = app
            .world_mut()
            .register_system(to_call(self.table.clone(), self.handles).pipe(self.system.clone()));
        let mut handlers = app.world_mut().get_resource_or_init::<Handlers<O>>();
        let index = usize::from(self.key.0);
        if handlers.0.len() <= index {
            handlers.0.resize(index + 1, None);
        }
        if let Some(slot) = handlers.0.get_mut(index) {
            *slot = Some(id);
        }
    }

    fn is_unique(&self) -> bool {
        false
    }
}
