use crate::answer_suggestion::answer_suggestion;
use crate::command_event::CommandEvent;
use crate::command_failure::{CommandFailure, CommandResult};
use crate::command_outbox::CommandOutbox;
use crate::command_queue::{QueuedLine, QueuedSuggestion};
use crate::command_targets::CommandTargets;
use crate::command_tree::CommandTree;
use crate::handlers::{CommandHandlers, ModifierHandlers};
use crate::invocation::Invocation;
use crate::modified::{Modified, ModifyResult};
use crate::parse_line::{LineToParse, parse_line};
use crate::registrar::Handler;
use crate::running_command::RunningCommand;
use crate::source_state::SourceState;
use crate::system_message::system_message;
use crate::worlds::Worlds;
use bevy_ecs::component::Mutable;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::In;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::SystemId;
use bevy_ecs::world::World;
use mcrs_command::{End, Executor, Failure, SourceLists, Stage, execute};
use mcrs_minecraft_engine::dim::deliver;
use mcrs_minecraft_protocol::text::{IntoText, Text};

/// The run of one command line. The line parses, and each stage call runs, in the world of the
/// dimension of its source, with the graph, the line, the targets and the outbox lent to that
/// world; what the call leaves in the outbox applies before the next call.
pub(crate) struct PassExecutor<'a, 'w> {
    pub(crate) host: &'a mut World,
    pub(crate) worlds: &'a mut [&'w mut World],
    pub(crate) tree: &'a mut CommandTree,
    pub(crate) targets: &'a mut CommandTargets,
    pub(crate) running: &'a mut RunningCommand,
    pub(crate) outbox: &'a mut CommandOutbox,
    pub(crate) output: Entity,
}

/// Swaps the values of the host and of `world`; a second swap gives them back.
fn lend(
    world: &mut World,
    tree: &mut CommandTree,
    targets: &mut CommandTargets,
    running: &mut RunningCommand,
    outbox: &mut CommandOutbox,
) -> bool {
    let present = world.contains_resource::<CommandTree>()
        && world.contains_resource::<CommandTargets>()
        && world.contains_resource::<RunningCommand>()
        && world.contains_resource::<CommandOutbox>();
    if present {
        swap_resource(world, tree);
        swap_resource(world, targets);
        swap_resource(world, running);
        swap_resource(world, outbox);
    }
    present
}

fn swap_resource<T: Resource<Mutability = Mutable>>(world: &mut World, value: &mut T) {
    if let Some(mut resource) = world.get_resource_mut::<T>() {
        std::mem::swap(&mut *resource, value);
    }
}

impl PassExecutor<'_, '_> {
    pub(crate) fn run_line(
        &mut self,
        queued: QueuedLine,
        stages: &mut Vec<Stage<Handler>>,
        lists: &mut SourceLists<SourceState>,
        limits: mcrs_command::Limits,
    ) {
        let Some(player) = self.targets.player(queued.player) else {
            return;
        };
        let (index, entity) = (player.dimension, player.entity);
        let mut input = LineToParse {
            queued,
            chain: std::mem::take(&mut self.running.chain),
        };
        let parsed = self
            .in_world(index, |world| {
                world.run_system_cached_with(parse_line, &mut input)
            })
            .and_then(|parsed| {
                parsed
                    .inspect_err(|error| tracing::error!("a command line did not parse: {error}"))
                    .ok()
            })
            .flatten();
        let LineToParse { queued, chain } = input;
        self.running.chain = chain;
        let Some(source) = parsed else {
            return;
        };
        let mut event = CommandEvent {
            player: entity,
            source,
            line: queued.line,
            cancelled: false,
        };
        if self
            .in_world(index, |world| world.trigger_ref(&mut event))
            .is_none()
            || event.cancelled
        {
            return;
        }
        let CommandEvent { source, line, .. } = event;
        self.running.line = line;
        stages.clear();
        stages.extend_from_slice(self.running.chain.stages());
        let end = execute(stages, source, limits, self, lists);
        if end == End::SequenceLimit {
            tracing::info!(
                "a command stopped at the limit of {} stages and runs",
                limits.sequence.max(1)
            );
        }
    }

    pub(crate) fn answer(&mut self, queued: &QueuedSuggestion) {
        let Some(index) = self
            .targets
            .player(queued.player)
            .map(|player| player.dimension)
        else {
            return;
        };
        let answered = self.in_world(index, |world| {
            world.run_system_cached_with(answer_suggestion, queued)
        });
        if let Some(Err(error)) = answered {
            tracing::error!("a suggestion request was not answered: {error}");
        }
    }

    fn invocation(&self, stage: usize, handler: &Handler, source: &SourceState) -> Invocation {
        Invocation {
            output: self.output,
            source: source.clone(),
            stage,
            branch: handler.branch,
        }
    }

    /// Runs `run` in the world at `index` with the host's values lent to it, then settles what
    /// the run left. `None` when there is no such world or it lacks the command resources.
    fn in_world<R>(&mut self, index: usize, run: impl FnOnce(&mut World) -> R) -> Option<R> {
        let world = self.worlds.get_mut(index)?;
        if !lend(world, self.tree, self.targets, self.running, self.outbox) {
            return None;
        }
        let result = run(world);
        lend(world, self.tree, self.targets, self.running, self.outbox);
        self.settle(index);
        Some(result)
    }

    /// Runs the system `pick` finds in the world of the source dimension, then settles what
    /// the run left. `None` when that world has no such system or the system did not run.
    fn call<O: 'static>(
        &mut self,
        invocation: Invocation,
        pick: impl FnOnce(&World) -> Option<SystemId<In<Invocation>, O>>,
    ) -> Option<O> {
        let index = self.targets.dimension_of(invocation.source.dimension)?;
        let system = pick(self.worlds.get(index)?)?;
        self.in_world(index, |world| world.run_system_with(system, invocation))?
            .inspect_err(|error| tracing::error!("a command handler did not run: {error}"))
            .ok()
    }

    /// Applies the effects of the run in the world at `index` in the order of writing, then
    /// sends its messages.
    fn settle(&mut self, index: usize) {
        let mut effects = std::mem::take(&mut self.outbox.effects);
        let mut worlds = Worlds {
            host: self.host,
            worlds: self.worlds,
            targets: self.targets,
        };
        for apply in effects.drain(..) {
            apply(&mut worlds, index);
        }
        self.outbox.effects = effects;
        for packet in self.outbox.packets.drain(..) {
            deliver(self.host, packet);
        }
        for broadcast in self.outbox.broadcasts.drain(..) {
            self.host.trigger(broadcast);
        }
    }
}

impl Executor<SourceState, Handler> for PassExecutor<'_, '_> {
    type Error = CommandFailure;

    fn modify(
        &mut self,
        stage: usize,
        modifier: &Handler,
        source: &SourceState,
        out: &mut Vec<SourceState>,
    ) -> Result<(), CommandFailure> {
        let invocation = self.invocation(stage, modifier, source);
        let key = modifier.key;
        let modified: Option<ModifyResult> = self.call(invocation, |world| {
            world.get_resource::<ModifierHandlers>()?.get(key)
        });
        let forked = &mut self.outbox.forked;
        let result = match modified.unwrap_or_else(|| Err(CommandFailure::failed())) {
            Ok(Modified::None) => Ok(()),
            Ok(Modified::One(source)) => {
                out.push(source);
                Ok(())
            }
            Ok(Modified::Forked) => {
                out.append(forked);
                Ok(())
            }
            Err(failure) => Err(failure),
        };
        forked.clear();
        result
    }

    fn run(
        &mut self,
        stage: usize,
        command: &Handler,
        source: &SourceState,
    ) -> Result<i32, CommandFailure> {
        let invocation = self.invocation(stage, command, source);
        let key = command.key;
        let result: CommandResult = self
            .call(invocation, |world| {
                world.get_resource::<CommandHandlers>()?.get(key)
            })
            .ok_or_else(CommandFailure::failed)?;
        result
    }

    fn report(&mut self, failure: Failure<CommandFailure>) {
        let failure = match failure {
            Failure::Command(failure) => failure,
            Failure::ForkLimit(limit) => {
                CommandFailure::translate("command.forkLimit", vec![limit.to_string().into_text()])
            }
        };
        let message: Text = failure.text();
        deliver(self.host, system_message(self.output, message));
    }
}
