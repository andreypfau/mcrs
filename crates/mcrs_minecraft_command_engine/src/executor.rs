use crate::command_failure::{CommandFailure, CommandResult};
use crate::command_outbox::CommandOutbox;
use crate::command_targets::CommandTargets;
use crate::handlers::{CommandHandlers, ModifierHandlers};
use crate::invocation::Invocation;
use crate::modified::{Modified, ModifyResult};
use crate::registrar::{Handler, HandlerWorld};
use crate::source_state::SourceState;
use crate::system_message::system_message;
use crate::worlds::Worlds;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::In;
use bevy_ecs::system::SystemId;
use bevy_ecs::world::World;
use mcrs_command::{Executor, Failure};
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine_world::owned_columns::{OwnedColumns, owns};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_text::Text;
use mcrs_voxel::ColumnPos;
use std::mem;

/// The sources of one stage that go on in the world of the dimension `to`.
pub(crate) struct Deferred {
    pub(crate) to: Entity,
    pub(crate) stage: usize,
    pub(crate) sources: Vec<SourceState>,
}

/// The run of one command line in the world of the dimension `here`. Each stage call runs here,
/// and what the call leaves in the outbox applies before the next call. A call whose handler
/// reads the world of the source dimension does not run when this world does not own the column
/// of the source: its source goes into `deferred`, to go on in the world that owns it.
pub(crate) struct DimExecutor<'a> {
    pub(crate) world: &'a mut World,
    pub(crate) here: Entity,
    pub(crate) output: PlayerSession,
    pub(crate) deferred: &'a mut Vec<Deferred>,
}

/// Where a stage call for a source runs.
enum Place {
    Here,
    Elsewhere(Entity),
}

impl DimExecutor<'_> {
    fn place(&self, handler: &Handler, source: &SourceState) -> Result<Place, CommandFailure> {
        let runs = self
            .world
            .get_resource::<CommandTargets>()
            .and_then(|targets| targets.dimension_of(source.dimension))
            .is_some();
        if !runs {
            return Err(CommandFailure::failed());
        }
        if handler.world == HandlerWorld::Any {
            return Ok(Place::Here);
        }
        // The halo of a cell lags its owner by a tick, so only the owner reads a column as a
        // world of one cell does.
        let owned = self.world.get_resource::<OwnedColumns>();
        if source.dimension != self.here || !owns(owned, ColumnPos::from(source.position)) {
            return Ok(Place::Elsewhere(source.dimension));
        }
        Ok(Place::Here)
    }

    fn defer(&mut self, to: Entity, stage: usize, source: &SourceState) {
        match self
            .deferred
            .iter_mut()
            .find(|deferred| deferred.to == to && deferred.stage == stage)
        {
            Some(deferred) => deferred.sources.push(source.clone()),
            None => self.deferred.push(Deferred {
                to,
                stage,
                sources: vec![source.clone()],
            }),
        }
    }

    fn invocation(&self, stage: usize, source: &SourceState) -> Invocation {
        Invocation {
            output: self.output,
            source: source.clone(),
            stage,
        }
    }

    /// Runs the system `pick` finds in this world, then settles what the run left. `None` when
    /// the world has no such system or the system did not run.
    fn call<O: 'static>(
        &mut self,
        invocation: Invocation,
        pick: impl FnOnce(&World) -> Option<SystemId<In<Invocation>, O>>,
    ) -> Option<O> {
        let system = pick(self.world)?;
        let result = self
            .world
            .run_system_with(system, invocation)
            .inspect_err(|error| tracing::error!("a command handler did not run: {error}"))
            .ok();
        settle(self.world);
        result
    }
}

/// Applies the effects that the last run left in the outbox of `world`, in the order of
/// writing, then sends its messages.
pub(crate) fn settle(world: &mut World) {
    let Some(mut outbox) = world.get_resource_mut::<CommandOutbox>() else {
        return;
    };
    let effects = mem::take(&mut outbox.effects);
    if !effects.is_empty()
        && let Some(mut targets) = world.remove_resource::<CommandTargets>()
    {
        let mut worlds = Worlds {
            world,
            targets: &mut targets,
        };
        for apply in effects {
            apply(&mut worlds);
        }
        world.insert_resource(targets);
    }
    let Some(mut outbox) = world.get_resource_mut::<CommandOutbox>() else {
        return;
    };
    let packets = mem::take(&mut outbox.packets);
    let broadcasts = mem::take(&mut outbox.broadcasts);
    if let Some(mut messages) = world.get_resource_mut::<Messages<OutboundPlayerPacket>>() {
        messages.write_batch(packets);
    }
    for broadcast in broadcasts {
        world.trigger(broadcast);
    }
}

impl Executor<SourceState, Handler> for DimExecutor<'_> {
    type Error = CommandFailure;

    fn modify(
        &mut self,
        stage: usize,
        modifier: &Handler,
        source: &SourceState,
        out: &mut Vec<SourceState>,
    ) -> Result<(), CommandFailure> {
        if let Place::Elsewhere(to) = self.place(modifier, source)? {
            self.defer(to, stage, source);
            return Ok(());
        }
        let invocation = self.invocation(stage, source);
        let (key, branch) = (modifier.key, modifier.branch);
        let modified: Option<ModifyResult> = self.call(invocation, |world| {
            world.get_resource::<ModifierHandlers>()?.get(key, branch)
        });
        let mut outbox = self.world.get_resource_mut::<CommandOutbox>();
        let result = match modified.unwrap_or_else(|| Err(CommandFailure::failed())) {
            Ok(Modified::None) => Ok(()),
            Ok(Modified::One(source)) => {
                out.push(source);
                Ok(())
            }
            Ok(Modified::Forked) => {
                if let Some(outbox) = outbox.as_mut() {
                    out.append(&mut outbox.forked);
                }
                Ok(())
            }
            Err(failure) => Err(failure),
        };
        if let Some(mut outbox) = outbox {
            outbox.forked.clear();
        }
        result
    }

    fn run(
        &mut self,
        stage: usize,
        command: &Handler,
        source: &SourceState,
    ) -> Result<i32, CommandFailure> {
        if let Place::Elsewhere(to) = self.place(command, source)? {
            self.defer(to, stage, source);
            return Ok(0);
        }
        let invocation = self.invocation(stage, source);
        let (key, branch) = (command.key, command.branch);
        let result: CommandResult = self
            .call(invocation, |world| {
                world.get_resource::<CommandHandlers>()?.get(key, branch)
            })
            .ok_or_else(CommandFailure::failed)?;
        result
    }

    fn report(&mut self, failure: Failure<CommandFailure>) {
        let failure = match failure {
            Failure::Command(failure) => failure,
            Failure::ForkLimit(limit) => {
                CommandFailure::translate("command.forkLimit", vec![limit.to_string().into()])
            }
        };
        let message: Text = failure.text();
        if let Some(packet) = system_message(self.output, &message) {
            self.world.write_message(packet);
        }
    }
}
