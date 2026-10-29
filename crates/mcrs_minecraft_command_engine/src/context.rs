use crate::command_failure::CommandFailure;
use crate::command_targets::CommandTargets;
use crate::command_tree::CommandTree;
use crate::invocation::Invocation;
use crate::running_command::RunningCommand;
use crate::source_state::SourceState;
use bevy_ecs::prelude::{Query, Res};
use bevy_ecs::system::SystemParam;
use mcrs_command::{Argument, ArgumentHandle, Contains};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_command::{Source, error_message};
use mcrs_minecraft_command_vanilla::{ArgumentError, VanillaArgument};
use mcrs_minecraft_entity::PLAYER_EYE_HEIGHT;
use mcrs_minecraft_level::world::dimension::DimensionTypeId;
use mcrs_minecraft_registries::CurrentRegistries;

/// What a handler reads its arguments with.
#[derive(SystemParam)]
pub struct CommandContext<'w, 's> {
    pub(crate) tree: Res<'w, CommandTree>,
    running: Res<'w, RunningCommand>,
    pub(crate) targets: Res<'w, CommandTargets>,
    pub(crate) registries: Option<Res<'w, CurrentRegistries>>,
    pub(crate) blocks: Option<Res<'w, Blocks>>,
    pub(crate) dimension_type: Query<'w, 's, &'static DimensionTypeId>,
}

impl<'w, 's> CommandContext<'w, 's> {
    /// The source of the call, which reads the world through this context.
    pub fn source(&self, call: &Invocation) -> Result<Source<'_, Self>, CommandFailure> {
        self.source_of(&call.source)
    }

    /// The source of `state`, with the rank of its dimension in the dimension order. The host
    /// fills the targets before a line parses, so a dimension they lack fails the source.
    pub(crate) fn source_of(
        &self,
        state: &SourceState,
    ) -> Result<Source<'_, Self>, CommandFailure> {
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
            ..Source::new(state.level, self)
        })
    }

    /// The dimensions that run and the players of every one of them.
    pub fn targets(&self) -> &CommandTargets {
        &self.targets
    }

    /// The block table of this world, when it has one.
    pub fn blocks(&self) -> Option<&Blocks> {
        self.blocks.as_deref()
    }

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
        VanillaArgument: Contains<T>,
        T: Argument<Source<'a, Self>>,
        ArgumentError: From<T::Error>,
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
        VanillaArgument: Contains<T>,
        T: Argument<Source<'a, Self>>,
        ArgumentError: From<T::Error>,
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
            Some(Err(error)) => Err(CommandFailure(error_message(
                line,
                &error.map_into::<ArgumentError>(),
            ))),
        }
    }
}
