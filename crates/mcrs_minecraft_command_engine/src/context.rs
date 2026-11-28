use crate::command_failure::CommandFailure;
use crate::command_set::{CommandSet, SetLookup};
use crate::command_targets::CommandTargets;
use crate::command_tree::{CommandTree, SetGraph};
use crate::invocation::Invocation;
use crate::running_command::RunningCommand;
use crate::source_state::SourceState;
use bevy_ecs::prelude::Res;
use bevy_ecs::system::{StaticSystemParam, SystemParam};
use mcrs_command::{Argument, ArgumentHandle, Contains};
use mcrs_minecraft_command::Source;
use mcrs_minecraft_pose::Pose;

#[derive(SystemParam)]
pub struct CommandContext<'w, 's, S: CommandSet> {
    tree: Res<'w, CommandTree<S>>,
    running: Res<'w, RunningCommand>,
    pub(crate) targets: Res<'w, CommandTargets>,
    lookup: StaticSystemParam<'w, 's, <S as CommandSet>::Lookup>,
}

impl<'w, 's, S: CommandSet> CommandContext<'w, 's, S> {
    pub fn source(
        &self,
        call: &Invocation,
    ) -> Result<Source<'_, SetLookup<'w, 's, S>>, CommandFailure> {
        self.source_of(&call.source)
    }

    pub(crate) fn source_of(
        &self,
        state: &SourceState,
    ) -> Result<Source<'_, SetLookup<'w, 's, S>>, CommandFailure> {
        let Some(dimension) = self.targets.dimension_of(state.dimension) else {
            tracing::error!(dimension = ?state.dimension, "a command source names no running dimension");
            return Err(CommandFailure::failed());
        };
        let eye_height = state
            .player
            .and_then(|player| self.targets.player(player))
            .map_or(Pose::Standing.player_dimensions().eye_height, |entry| {
                entry.dimensions.eye_height
            });
        Ok(Source {
            position: state.position,
            yaw: state.yaw,
            pitch: state.pitch,
            anchor: state.anchor,
            eye_height: Some(f64::from(eye_height)),
            dimension,
            ..Source::new(state.level, self.lookup())
        })
    }

    pub fn lookup(&self) -> &SetLookup<'w, 's, S> {
        &self.lookup
    }

    pub(crate) fn graph(&self) -> &SetGraph<S> {
        self.tree.graph()
    }

    pub fn targets(&self) -> &CommandTargets {
        &self.targets
    }

    pub fn line(&self) -> &str {
        &self.running.line
    }

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
        let graph = self.tree.graph();
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
