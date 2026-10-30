use crate::call::{Call, to_call};
use crate::command_failure::CommandResult;
use crate::command_set::CommandSet;
use crate::command_tree::CommandTreeBuilder;
use crate::effect_queue::EffectPlugin;
use crate::handlers::{HandlerKey, Handlers};
use crate::modified::ModifyResult;
use crate::registrar::{BranchUse, CommandRegistrar};
use crate::worlds::Worlds;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use mcrs_command::BuildError;
use mcrs_minecraft_level::world::sub_app::AddDimensionPlugins;
use std::marker::PhantomData;

/// Why a command, or one of its nodes, is not in the command tree.
#[derive(Debug, thiserror::Error)]
pub enum RegistrationError {
    /// A node of the command conflicts with the tree; nothing of the command is added.
    #[error("the command tree does not take the command: {0}")]
    Tree(#[from] BuildError),
    /// The app has no tree to add to: the [`CommandPlugin`](crate::CommandPlugin) of the set is
    /// not added, or the app already finished.
    #[error("commands are added after CommandPlugin and before the app finishes")]
    NoTree,
    /// The tree holds as many commands as a handler key can count.
    #[error("the command tree holds as many handlers as a handler key can count")]
    TooManyCommands,
    /// A command has more branches than a branch index can count.
    #[error("the command has more branches than a branch index can count")]
    TooManyBranches,
    /// A branch redirects through a modifier, and the command registers no modifier handler.
    #[error("a branch of the command has a modifier and the command has no modifier handler")]
    UnhandledModifier,
}

/// Adds commands and command effects to an [`App`] that has the
/// [`CommandPlugin`](crate::CommandPlugin).
pub trait AppCommandsExt {
    /// Registers a command in the tree of the set `A`: `build` adds its nodes and returns the
    /// handles its handler reads, and every dimension world runs `handler` when one of its
    /// branches runs. Nothing is registered when `build` returns an error.
    fn add_command<A, B, C, S, M>(
        &mut self,
        build: impl FnOnce(&mut CommandRegistrar<'_, A, B>) -> Result<C, BuildError>,
        handler: S,
    ) -> Result<&mut Self, RegistrationError>
    where
        A: CommandSet,
        B: Copy + PartialEq + Send + Sync + 'static,
        C: Copy + Send + Sync + 'static,
        S: IntoSystem<In<Call<B, C>>, CommandResult, M> + Clone + Send + Sync + 'static,
        M: 'static;

    /// Registers a command whose redirects have modifiers: every dimension world runs
    /// `modify` for a source when a stage of a chain follows one of them.
    fn add_modifying_command<A, B, C, S, M, D, N>(
        &mut self,
        build: impl FnOnce(&mut CommandRegistrar<'_, A, B>) -> Result<C, BuildError>,
        handler: S,
        modify: D,
    ) -> Result<&mut Self, RegistrationError>
    where
        A: CommandSet,
        B: Copy + PartialEq + Send + Sync + 'static,
        C: Copy + Send + Sync + 'static,
        S: IntoSystem<In<Call<B, C>>, CommandResult, M> + Clone + Send + Sync + 'static,
        M: 'static,
        D: IntoSystem<In<Call<B, C>>, ModifyResult, N> + Clone + Send + Sync + 'static,
        N: 'static;

    /// Registers a command effect type: every dimension world gets an
    /// [`EffectQueue<E>`](crate::EffectQueue), and the host applies each effect that a
    /// handler writes with `apply`, in the order of writing across all effect types. A later
    /// registration of the same type replaces `apply`.
    fn add_command_effect<E: Send + Sync + 'static>(
        &mut self,
        apply: fn(&mut Worlds<'_, '_>, E),
    ) -> &mut Self;
}

/// The nodes of one command in the tree: the handles its builder returned, its key and its
/// branches.
struct Registration<B, C> {
    handles: C,
    key: HandlerKey,
    branches: Vec<BranchUse<B>>,
}

/// Whether a command registers a handler for its modifiers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Modifiers {
    Unhandled,
    Handled,
}

/// Adds the nodes of one command to the tree, or nothing when `build` returns an error or a
/// branch has a modifier that nothing handles. `None` when the command has no executable node.
fn register<A, B, C>(
    app: &mut App,
    modifiers: Modifiers,
    build: impl FnOnce(&mut CommandRegistrar<'_, A, B>) -> Result<C, BuildError>,
) -> Result<Option<Registration<B, C>>, RegistrationError>
where
    A: CommandSet,
    B: Copy + PartialEq,
{
    let mut tree = app
        .world_mut()
        .get_resource_mut::<CommandTreeBuilder<A>>()
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
        overflowed: false,
    };
    let built = build(&mut registrar);
    if registrar.overflowed {
        tree.builder = before;
        return Err(RegistrationError::TooManyBranches);
    }
    if modifiers == Modifiers::Unhandled
        && registrar.branches.iter().any(|used| used.tables.modifies())
    {
        tree.builder = before;
        return Err(RegistrationError::UnhandledModifier);
    }
    let handles = match built {
        Ok(handles) => handles,
        Err(error) => {
            tree.builder = before;
            return Err(error.into());
        }
    };
    let branches = registrar.branches;
    tree.next = next;
    if branches.is_empty() {
        return Ok(None);
    }
    Ok(Some(Registration {
        handles,
        key,
        branches,
    }))
}

impl AppCommandsExt for App {
    fn add_command<A, B, C, S, M>(
        &mut self,
        build: impl FnOnce(&mut CommandRegistrar<'_, A, B>) -> Result<C, BuildError>,
        handler: S,
    ) -> Result<&mut Self, RegistrationError>
    where
        A: CommandSet,
        B: Copy + PartialEq + Send + Sync + 'static,
        C: Copy + Send + Sync + 'static,
        S: IntoSystem<In<Call<B, C>>, CommandResult, M> + Clone + Send + Sync + 'static,
        M: 'static,
    {
        let Some(Registration {
            handles,
            key,
            branches,
        }) = register(self, Modifiers::Unhandled, build)?
        else {
            return Ok(self);
        };
        Ok(self.add_dimension_plugin(HandlerPlugin {
            handles,
            key,
            branches: branches
                .iter()
                .map(|used| used.tables.runs().then_some(used.branch))
                .collect(),
            system: handler,
            marker: PhantomData::<fn() -> (M, CommandResult)>,
        }))
    }

    fn add_modifying_command<A, B, C, S, M, D, N>(
        &mut self,
        build: impl FnOnce(&mut CommandRegistrar<'_, A, B>) -> Result<C, BuildError>,
        handler: S,
        modify: D,
    ) -> Result<&mut Self, RegistrationError>
    where
        A: CommandSet,
        B: Copy + PartialEq + Send + Sync + 'static,
        C: Copy + Send + Sync + 'static,
        S: IntoSystem<In<Call<B, C>>, CommandResult, M> + Clone + Send + Sync + 'static,
        M: 'static,
        D: IntoSystem<In<Call<B, C>>, ModifyResult, N> + Clone + Send + Sync + 'static,
        N: 'static,
    {
        let Some(Registration {
            handles,
            key,
            branches,
        }) = register(self, Modifiers::Handled, build)?
        else {
            return Ok(self);
        };
        self.add_dimension_plugin(HandlerPlugin {
            handles,
            key,
            branches: branches
                .iter()
                .map(|used| used.tables.runs().then_some(used.branch))
                .collect(),
            system: handler,
            marker: PhantomData::<fn() -> (M, CommandResult)>,
        });
        Ok(self.add_dimension_plugin(HandlerPlugin {
            handles,
            key,
            branches: branches
                .iter()
                .map(|used| used.tables.modifies().then_some(used.branch))
                .collect(),
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
    branches: Vec<Option<B>>,
    system: S,
    marker: PhantomData<fn() -> M>,
}

impl<B: Clone, C: Clone, S: Clone, M> Clone for HandlerPlugin<B, C, S, M> {
    fn clone(&self) -> Self {
        Self {
            handles: self.handles.clone(),
            key: self.key,
            branches: self.branches.clone(),
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
        let ids: Vec<_> = self
            .branches
            .iter()
            .map(|branch| {
                branch.map(|branch| {
                    app.world_mut()
                        .register_system(to_call(branch, self.handles).pipe(self.system.clone()))
                })
            })
            .collect();
        app.world_mut()
            .get_resource_or_init::<Handlers<O>>()
            .insert(self.key, ids);
    }

    fn is_unique(&self) -> bool {
        false
    }
}
