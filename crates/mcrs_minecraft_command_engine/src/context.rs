use crate::command_failure::CommandFailure;
use crate::command_set::{CommandSet, SetLookup};
use crate::command_targets::CommandTargets;
use crate::command_tree::CommandTree;
use crate::invocation::Invocation;
use crate::running_command::RunningCommand;
use crate::source_state::SourceState;
use bevy_ecs::prelude::Res;
use bevy_ecs::system::{StaticSystemParam, SystemParam};
use mcrs_command::{Argument, ArgumentHandle, Contains};
use mcrs_minecraft_command::Source;
use mcrs_minecraft_entity::PLAYER_EYE_HEIGHT;

/// What a handler of a command of the set `S` reads its arguments with.
#[derive(SystemParam)]
pub struct CommandContext<'w, 's, S: CommandSet> {
    pub(crate) tree: Res<'w, CommandTree<S>>,
    running: Res<'w, RunningCommand>,
    pub(crate) targets: Res<'w, CommandTargets>,
    lookup: StaticSystemParam<'w, 's, <S as CommandSet>::Lookup>,
}

impl<'w, 's, S: CommandSet> CommandContext<'w, 's, S> {
    /// The source of the call, which reads the world through the lookup of the set.
    pub fn source(
        &self,
        call: &Invocation,
    ) -> Result<Source<'_, SetLookup<'w, 's, S>>, CommandFailure> {
        self.source_of(&call.source)
    }

    /// The source of `state`, with the rank of its dimension in the dimension order. The host
    /// fills the targets before a line parses, so a dimension they lack fails the source.
    pub(crate) fn source_of(
        &self,
        state: &SourceState,
    ) -> Result<Source<'_, SetLookup<'w, 's, S>>, CommandFailure> {
        let Some(dimension) = self.targets.dimension_of(state.dimension) else {
            tracing::error!(dimension = ?state.dimension, "a command source names no running dimension");
            return Err(CommandFailure::failed());
        };
        Ok(Source {
            position: state.position,
            yaw: state.yaw,
            pitch: state.pitch,
            anchor: state.anchor,
            eye_height: Some(PLAYER_EYE_HEIGHT),
            dimension,
            ..Source::new(state.level, self.lookup())
        })
    }

    /// What the arguments of the set read from this world.
    pub fn lookup(&self) -> &SetLookup<'w, 's, S> {
        &self.lookup
    }

    /// The dimensions that run and the players of every one of them.
    pub fn targets(&self) -> &CommandTargets {
        &self.targets
    }

    /// The command line that runs, without its leading slash.
    pub fn line(&self) -> &str {
        &self.running.line
    }

    /// The value of an argument the branch of the call always holds.
    pub fn arg<'a, T>(
        &'a self,
        call: &Invocation,
        arg: ArgumentHandle<T>,
    ) -> Result<T::Value<'a>, CommandFailure>
    where
        S::Argument: Contains<T>,
        T: Argument<Source<'a, SetLookup<'w, 's, S>>>,
        S::Error: From<T::Error>,
    {
        self.opt(call, arg)?.ok_or_else(CommandFailure::failed)
    }

    /// The value of an argument the branch of the call may lack.
    pub fn opt<'a, T>(
        &'a self,
        call: &Invocation,
        arg: ArgumentHandle<T>,
    ) -> Result<Option<T::Value<'a>>, CommandFailure>
    where
        S::Argument: Contains<T>,
        T: Argument<Source<'a, SetLookup<'w, 's, S>>>,
        S::Error: From<T::Error>,
    {
        let Some(span) = self.running.chain.span(call.stage, arg.node()) else {
            return Ok(None);
        };
        let graph = self.tree.graph().ok_or_else(CommandFailure::failed)?;
        let source = self.source(call)?;
        let line = self.line();
        match graph.value(line, span, arg, &source) {
            None => Err(CommandFailure::failed()),
            Some(Ok(value)) => Ok(Some(value)),
            Some(Err(error)) => Err(CommandFailure(mcrs_minecraft_command::error_message(
                line,
                &error.map_into::<S::Error>(),
            ))),
        }
    }
}
