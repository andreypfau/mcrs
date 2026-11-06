use crate::answer_suggestion::answer_suggestion;
use crate::command_event::CommandEvent;
use crate::command_limits::CommandLimits;
use crate::command_queue::{CommandQueue, QueuedLine};
use crate::command_set::CommandSet;
use crate::command_targets::CommandTargets;
use crate::dimension_entry::DimensionEntry;
use crate::executor::{Deferred, DimExecutor, settle};
use crate::parse_line::{LineToParse, ParseSource, parse_line};
use crate::player_entry::PlayerEntry;
use crate::registrar::Handler;
use crate::running_command::RunningCommand;
use crate::source_state::SourceState;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_ecs::query::QueryState;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_ecs::world::World;
use mcrs_command::{End, Limits, Resume, SourceLists, Stage};
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::command_handoff::CommandHandoff;
use mcrs_minecraft_engine::coordinator::running_dimensions::RunningDimensions;
use mcrs_minecraft_engine::dim_mail::DimMail;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::player_order::DepartureRequests;
use mcrs_minecraft_engine::player_roster::ForeignPlayers;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::sub_app::DimensionLabel;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::Id;

/// The buffers of the command runs of a world, reused from one tick to the next.
#[derive(Resource, Default)]
pub(crate) struct PassBuffers {
    stages: Vec<Stage<Handler>>,
    lists: SourceLists<SourceState>,
    queue: CommandQueue,
    deferred: Vec<Deferred>,
    players: Option<QueryState<PlayerRow, With<Player>>>,
}

type PlayerRow = (
    Entity,
    &'static Owner,
    &'static GameProfile,
    &'static Transform,
    &'static PlayerGameMode,
);

/// Whether this world has commands to run in this tick.
pub(crate) fn commands_pending(
    queue: Res<CommandQueue>,
    handoffs: Option<Res<Messages<CommandHandoff>>>,
) -> bool {
    !queue.is_empty() || handoffs.is_some_and(|handoffs| !handoffs.is_empty())
}

/// Runs the commands of this tick at the sync point, after the input of the tick reached the
/// players: first the runs that other worlds handed over, in the order they came, then the
/// command lines of the players of this world in the order they arrived, then the suggestion
/// requests.
pub(crate) fn run_commands<S: CommandSet>(world: &mut World) {
    let Some(mut buffers) = world.remove_resource::<PassBuffers>() else {
        return;
    };
    let Some(here) = world.get_resource::<DimensionLabel>().map(|label| label.0) else {
        world.insert_resource(buffers);
        return;
    };
    let limits = world
        .get_resource::<CommandLimits>()
        .copied()
        .unwrap_or_default()
        .0;
    if let Some(mut received) = world.get_resource_mut::<CommandQueue>() {
        std::mem::swap(&mut *received, &mut buffers.queue);
    }
    let handoffs: Vec<CommandHandoff> = world
        .get_resource_mut::<Messages<CommandHandoff>>()
        .map(|mut handoffs| handoffs.drain().collect())
        .unwrap_or_default();
    collect_targets(world, &mut buffers, here);
    let mut run = Run {
        world,
        here,
        limits,
        stages: &mut buffers.stages,
        lists: &mut buffers.lists,
        deferred: &mut buffers.deferred,
    };
    for handoff in handoffs {
        run.handoff::<S>(handoff);
    }
    for queued in buffers.queue.lines.drain(..) {
        let line = run.line::<S>(queued);
        if line.capacity() > 0 {
            buffers.queue.spare.push(line);
        }
    }
    for queued in buffers.queue.suggestions.drain(..) {
        let answered = run
            .world
            .run_system_cached_with(answer_suggestion::<S>, &queued);
        if let Err(error) = answered {
            tracing::error!("a suggestion request was not answered: {error}");
        }
        settle(run.world);
    }
    if let Some(mut received) = world.get_resource_mut::<CommandQueue>() {
        received.spare.append(&mut buffers.queue.spare);
    }
    world.insert_resource(buffers);
}

/// The world and the buffers of the runs of one tick.
struct Run<'a> {
    world: &'a mut World,
    here: Entity,
    limits: Limits,
    stages: &'a mut Vec<Stage<Handler>>,
    lists: &'a mut SourceLists<SourceState>,
    deferred: &'a mut Vec<Deferred>,
}

impl Run<'_> {
    /// Parses a line for `source` into the running chain, and sends the parse errors. Gives
    /// the line back, and the source when the line runs.
    fn parse<S: CommandSet>(
        &mut self,
        player: PlayerSession,
        line: String,
        source: ParseSource,
    ) -> (String, Option<SourceState>) {
        let chain = self
            .world
            .get_resource_mut::<RunningCommand>()
            .map(|mut running| std::mem::take(&mut running.chain))
            .unwrap_or_default();
        let mut input = LineToParse {
            player,
            line,
            source,
            chain,
        };
        let parsed = self
            .world
            .run_system_cached_with(parse_line::<S>, &mut input);
        let LineToParse { line, chain, .. } = input;
        if let Some(mut running) = self.world.get_resource_mut::<RunningCommand>() {
            running.chain = chain;
        }
        settle(self.world);
        match parsed {
            Ok(source) => (line, source),
            Err(error) => {
                tracing::error!("a command line did not parse: {error}");
                (line, None)
            }
        }
    }

    /// Runs a line of a player of this world. Gives the line back for its buffer.
    fn line<S: CommandSet>(&mut self, queued: QueuedLine) -> String {
        let QueuedLine { player, line } = queued;
        let entity = self
            .world
            .get_resource::<CommandTargets>()
            .and_then(|targets| targets.player(player))
            .and_then(|entry| entry.entity);
        let Some(entity) = entity else {
            return line;
        };
        let (line, source) = self.parse::<S>(player, line, ParseSource::Player);
        let Some(source) = source else {
            return line;
        };
        let mut event = CommandEvent {
            player: entity,
            source,
            line,
            cancelled: false,
        };
        self.world.trigger_ref(&mut event);
        settle(self.world);
        if event.cancelled {
            return event.line;
        }
        let CommandEvent { source, line, .. } = event;
        let start = Resume {
            stage: 0,
            sources: vec![source.clone()],
            cost: 0,
        };
        self.chain(player, line, source, start)
    }

    /// Runs on the part of a run that another world handed over.
    fn handoff<S: CommandSet>(&mut self, handoff: CommandHandoff) {
        let CommandHandoff {
            output,
            line,
            origin,
            stage,
            sources,
            cost,
            ..
        } = handoff;
        let (line, parsed) = self.parse::<S>(output, line, ParseSource::Given(origin));
        let Some(origin) = parsed else {
            return;
        };
        let start = Resume {
            stage,
            sources,
            cost,
        };
        self.chain(output, line, origin, start);
    }

    /// Runs the running chain from `start`, then hands the sources that reached another world
    /// to that world. Gives the line back.
    fn chain(
        &mut self,
        output: PlayerSession,
        line: String,
        origin: SourceState,
        start: Resume<SourceState>,
    ) -> String {
        let Some(mut running) = self.world.get_resource_mut::<RunningCommand>() else {
            return line;
        };
        running.line = line;
        self.stages.clear();
        self.stages.extend_from_slice(running.chain.stages());
        self.deferred.clear();
        let first = start.stage;
        let spent = start.cost;
        let mut executor = DimExecutor {
            world: self.world,
            here: self.here,
            output,
            deferred: self.deferred,
        };
        let end = mcrs_command::resume(self.stages, start, self.limits, &mut executor, self.lists);
        if end == End::SequenceLimit {
            tracing::info!(
                "a command stopped at the limit of {} stages and runs",
                self.limits.sequence.max(1)
            );
        }
        let line = self
            .world
            .get_resource_mut::<RunningCommand>()
            .map(|mut running| std::mem::take(&mut running.line))
            .unwrap_or_default();
        self.hand_off(output, &line, &origin, first, spent);
        line
    }

    /// Sends each group of deferred sources to its world. A group of the run stage costs one
    /// run for each source, which the stages here already counted against the limit.
    fn hand_off(
        &mut self,
        output: PlayerSession,
        line: &str,
        origin: &SourceState,
        first: usize,
        spent: usize,
    ) {
        let last = self.stages.len().saturating_sub(1);
        let budget = self.limits.sequence.max(1);
        for Deferred { to, stage, sources } in self.deferred.drain(..) {
            let cost = if stage == last {
                budget.saturating_sub(sources.len())
            } else {
                let modifiers = self.stages.get(first..stage).map_or(0, |before| {
                    before
                        .iter()
                        .filter(|stage| stage.modifier.is_some())
                        .count()
                });
                spent.saturating_add(modifiers)
            };
            self.world
                .get_resource_or_init::<DimMail>()
                .hand_off(CommandHandoff {
                    to,
                    output,
                    line: line.to_owned(),
                    origin: origin.clone(),
                    stage,
                    sources,
                    cost,
                });
        }
    }
}

/// Fills the targets of this tick: the dimensions that run, the players of this world as they
/// are, and the players of the other worlds as those worlds reported them.
fn collect_targets(world: &mut World, buffers: &mut PassBuffers, here: Entity) {
    let Some(mut targets) = world.remove_resource::<CommandTargets>() else {
        return;
    };
    fill_dimensions(world, &mut targets);
    let own = targets.dimension_of(here);
    let players = buffers
        .players
        .get_or_insert_with(|| world.query_filtered::<PlayerRow, With<Player>>());
    let requests = world.get_resource::<DepartureRequests>();
    targets.players.clear();
    for (entity, &Owner(session), profile, transform, mode) in players.iter(world) {
        let dimension = requests
            .and_then(|requests| requests.destination(entity))
            .and_then(|key| {
                targets
                    .dimensions
                    .iter()
                    .position(|entry| entry.key == *key)
            })
            .or(own);
        let Some(dimension) = dimension else {
            continue;
        };
        targets.players.push(PlayerEntry {
            session,
            dimension,
            entity: Some(entity),
            uuid: profile.id,
            name: profile.username.clone(),
            transform: *transform,
            mode: mode.0,
        });
    }
    if let Some(foreign) = world.get_resource::<ForeignPlayers>() {
        for row in foreign.rows() {
            let Some(dimension) = targets.dimension_of(row.dim) else {
                continue;
            };
            targets.players.push(PlayerEntry {
                session: row.session,
                dimension,
                entity: None,
                uuid: row.uuid,
                name: row.name.clone(),
                transform: row.transform,
                mode: row.mode,
            });
        }
    }
    targets.index_players();
    world.insert_resource(targets);
}

/// The running dimensions in the dimension order, with the scale of each dimension type.
fn fill_dimensions(world: &World, targets: &mut CommandTargets) {
    targets.dimensions.clear();
    let (Some(running), Some(types)) = (
        world.get_resource::<RunningDimensions>(),
        world
            .get_resource::<CurrentRegistries>()
            .and_then(|registries| registries.registry::<DimensionTypeFile>()),
    ) else {
        return;
    };
    targets
        .dimensions
        .extend(running.iter().filter_map(|dimension| {
            let id = Id::checked(dimension.dimension_type.number(), types.len())?;
            Some(DimensionEntry {
                label: dimension.label,
                key: dimension.key.clone(),
                coordinate_scale: types.get(id)?.coordinate_scale,
            })
        }));
}
