use crate::command::effect::CommandOutbox;
use crate::command::run::{CommandEvent, LineToParse, answer_suggestion, parse_line};
use crate::command::targets::{DimensionEntry, PlayerEntry, in_dimension_order};
use crate::command::{
    CommandFailure, CommandHandlers, CommandLimits, CommandResult, CommandTargets, CommandTree,
    Handler, Invocation, Modified, ModifierHandlers, ModifyResult, RunningCommand, SourceState,
    system_message,
};
use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::PlayerGameMode;
use crate::world::entity::player::dimension_change::{
    Arrival, DimensionChangeError, change_dimension,
};
use bevy_app::{App, SubApp};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{DetectChangesMut, In, With};
use bevy_ecs::query::QueryState;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::SystemId;
use bevy_ecs::world::{World, WorldId};
use mcrs_command::{End, Executor, Failure, SourceLists, Stage, execute};
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::dim::deliver;
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::{Place, Session, SessionPlacement};
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeId};
use mcrs_minecraft_level::world::sub_app::DimensionLabel;
use mcrs_minecraft_protocol::text::{IntoText, Text};
use mcrs_minecraft_protocol::{GameMode, VarInt};
use mcrs_minecraft_registries::CurrentRegistries;

/// The command lines and the suggestion requests that the host received since the last pass.
#[derive(Resource, Default)]
pub(crate) struct CommandQueue {
    pub(crate) lines: Vec<QueuedLine>,
    pub(crate) suggestions: Vec<QueuedSuggestion>,
}

impl CommandQueue {
    fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.suggestions.is_empty()
    }
}

/// A command line in the order of arrival, from the player with the host anchor `player`.
pub(crate) struct QueuedLine {
    pub(crate) player: Entity,
    pub(crate) line: String,
}

/// The latest suggestion request of the player with the host anchor `player`.
pub(crate) struct QueuedSuggestion {
    pub(crate) player: Entity,
    pub(crate) id: VarInt,
    pub(crate) text: String,
}

/// The host's buffers for running command lines, reused from one run to the next.
#[derive(Resource, Default)]
pub(crate) struct CommandSeam {
    tree: CommandTree,
    targets: CommandTargets,
    running: RunningCommand,
    outbox: CommandOutbox,
    stages: Vec<Stage<Handler>>,
    lists: SourceLists<SourceState>,
    queue: CommandQueue,
    packets: Vec<OutboundPlayerPacket>,
    queries: Vec<WorldQueries>,
}

type PlayerRow = (
    Entity,
    &'static HostAnchor,
    &'static GameProfile,
    &'static Transform,
    &'static PlayerGameMode,
);

type DimensionRow = (
    &'static ResourceKey<mcrs_minecraft_dimension::Dimension>,
    &'static DimensionTypeId,
);

/// The queries the seam runs in one dimension world, kept from one pass to the next.
struct WorldQueries {
    world: WorldId,
    players: QueryState<PlayerRow, With<Player>>,
    dimension: QueryState<DimensionRow, With<Dimension>>,
}

impl WorldQueries {
    /// The index of the queries of `world`, built on its first pass.
    fn index(queries: &mut Vec<WorldQueries>, world: &mut World) -> usize {
        let id = world.id();
        queries
            .iter()
            .position(|query| query.world == id)
            .unwrap_or_else(|| {
                queries.push(WorldQueries {
                    world: id,
                    players: world.query_filtered(),
                    dimension: world.query_filtered(),
                });
                queries.len() - 1
            })
    }
}

/// Parses and runs the command lines the host received, one after another in the order they
/// arrived, then answers the suggestion requests, with every dimension world in reach. The
/// server calls it between two ticks, after the dimensions hand over their packets; an embedder
/// that drives the app itself calls it at the same point.
pub fn run_pending_commands(app: &mut App) {
    let sub_apps = app.sub_apps_mut();
    let host = sub_apps.main.world_mut();
    if host
        .get_resource::<CommandQueue>()
        .is_none_or(CommandQueue::is_empty)
    {
        return;
    }
    let Some(mut seam) = host.remove_resource::<CommandSeam>() else {
        return;
    };
    let limits = host
        .get_resource::<CommandLimits>()
        .copied()
        .unwrap_or_default()
        .0;
    let CommandSeam {
        tree,
        targets,
        running,
        outbox,
        stages,
        lists,
        queue,
        packets,
        queries,
    } = &mut seam;
    if let Some(mut received) = host.get_resource_mut::<CommandQueue>() {
        std::mem::swap(&mut *received, queue);
    }
    if let Some(mut owned) = host.get_resource_mut::<CommandTree>() {
        std::mem::swap(&mut *owned, tree);
    }
    queries.retain(|query| {
        sub_apps
            .sub_apps
            .values()
            .any(|sub| sub.world().id() == query.world)
    });
    let found = in_dimension_order(
        sub_apps.sub_apps.values_mut().map(SubApp::world_mut),
        |world| {
            let index = WorldQueries::index(queries, world);
            let entry = dimension_entry(world, queries.get_mut(index)?)?;
            Some((entry, index))
        },
        |(entry, _)| &entry.key,
    );
    targets.dimensions.clear();
    let mut query_of = Vec::with_capacity(found.len());
    let mut worlds: Vec<&mut World> = found
        .into_iter()
        .map(|((entry, index), world)| {
            targets.dimensions.push(entry);
            query_of.push(index);
            world
        })
        .collect();
    collect_players(host, &worlds, &query_of, queries, targets);
    let mut executor = SeamExecutor {
        host: &mut *host,
        worlds: &mut worlds,
        tree: &mut *tree,
        targets: &mut *targets,
        running: &mut *running,
        outbox: &mut *outbox,
        packets: &mut *packets,
        output: Entity::PLACEHOLDER,
    };
    for queued in queue.lines.drain(..) {
        executor.output = queued.player;
        executor.run_line(queued, stages, lists, limits);
    }
    for queued in queue.suggestions.drain(..) {
        executor.output = queued.player;
        executor.answer(&queued);
    }
    if let Some(mut owned) = host.get_resource_mut::<CommandTree>() {
        std::mem::swap(&mut *owned, tree);
    }
    host.insert_resource(seam);
}

fn dimension_entry(world: &World, queries: &mut WorldQueries) -> Option<DimensionEntry> {
    let label = world.get_resource::<DimensionLabel>()?.0;
    let (key, type_id) = queries
        .dimension
        .single(world)
        .ok()
        .map(|(key, type_id)| (key.clone(), type_id.0))?;
    let types = world
        .get_resource::<CurrentRegistries>()?
        .registry::<DimensionTypeFile>()?;
    let id = mcrs_minecraft_core::Id::checked(type_id.number(), types.len())?;
    let coordinate_scale = types.get(id)?.coordinate_scale;
    Some(DimensionEntry {
        label,
        key,
        coordinate_scale,
    })
}

/// Every player that its session places in the world that holds it, in the order of the
/// player list. A player that leaves has no place, so no command finds it. The entries keep
/// their names' capacity from one pass to the next.
fn collect_players(
    host: &World,
    worlds: &[&mut World],
    query_of: &[usize],
    queries: &mut [WorldQueries],
    targets: &mut CommandTargets,
) {
    let mut filled = 0;
    for (dimension, world) in worlds.iter().enumerate() {
        let Some(label) = targets.dimensions.get(dimension).map(|entry| entry.label) else {
            continue;
        };
        let Some(query) = query_of
            .get(dimension)
            .and_then(|&index| queries.get_mut(index))
        else {
            continue;
        };
        for (entity, &HostAnchor(anchor), profile, transform, mode) in query.players.iter(world) {
            let (Some(session), Some(placement)) = (
                host.get::<Session>(anchor),
                host.get::<SessionPlacement>(anchor),
            ) else {
                continue;
            };
            let placed = match placement.place() {
                Place::InDim(dim) | Place::Joining(dim) => dim == label,
                Place::Unplaced => false,
            };
            if !placed {
                continue;
            }
            let entry = PlayerEntry {
                anchor,
                dimension,
                entity,
                uuid: profile.id,
                name: String::new(),
                transform: *transform,
                mode: mode.0,
                list_order: session.0.0,
            };
            match targets.players.get_mut(filled) {
                Some(reused) => {
                    let name = std::mem::take(&mut reused.name);
                    *reused = PlayerEntry { name, ..entry };
                    reused.name.clone_from(&profile.username);
                }
                None => targets.players.push(PlayerEntry {
                    name: profile.username.clone(),
                    ..entry
                }),
            }
            filled += 1;
        }
    }
    targets.players.truncate(filled);
    targets.index_players();
}

/// The run of one command line. The line parses, and each stage call runs, in the world of the
/// dimension of its source, with the graph, the line, the targets and the outbox lent to that
/// world; what the call leaves in the outbox applies before the next call.
struct SeamExecutor<'a, 'w> {
    host: &'a mut World,
    worlds: &'a mut [&'w mut World],
    tree: &'a mut CommandTree,
    targets: &'a mut CommandTargets,
    running: &'a mut RunningCommand,
    outbox: &'a mut CommandOutbox,
    packets: &'a mut Vec<OutboundPlayerPacket>,
    output: Entity,
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
        std::mem::swap(&mut *world.resource_mut::<CommandTree>(), tree);
        std::mem::swap(&mut *world.resource_mut::<CommandTargets>(), targets);
        std::mem::swap(&mut *world.resource_mut::<RunningCommand>(), running);
        std::mem::swap(&mut *world.resource_mut::<CommandOutbox>(), outbox);
    }
    present
}

impl SeamExecutor<'_, '_> {
    fn run_line(
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
        let Some(world) = self.worlds.get_mut(index) else {
            return;
        };
        let mut event = CommandEvent {
            player: entity,
            source,
            line: queued.line,
            cancelled: false,
        };
        world.trigger_ref(&mut event);
        if event.cancelled {
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

    fn answer(&mut self, queued: &QueuedSuggestion) {
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
            packets: self.packets,
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

/// Why an operation of [`Worlds`] changed no player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlayerChangeError {
    #[error("no online player has this host anchor")]
    NotOnline,
    #[error("the world of the player does not hold its entity")]
    NotInWorld,
    #[error("no dimension runs at this index")]
    NoDimension,
    #[error("the player did not change dimension: {0}")]
    DimensionChange(DimensionChangeError),
}

/// The host and every dimension world, lent to the appliers of the effects of one run, between
/// two ticks. A player of [`CommandTargets`] changes only through [`Worlds::set_transform`],
/// [`Worlds::change_dimension`] and [`Worlds::set_game_mode`], which keep its entry current for
/// the later effects, stages and lines of the pass.
pub struct Worlds<'s, 'w> {
    host: &'s mut World,
    worlds: &'s mut [&'w mut World],
    targets: &'s mut CommandTargets,
    packets: &'s mut Vec<OutboundPlayerPacket>,
}

impl Worlds<'_, '_> {
    /// The dimensions in the dimension order and the online players, as the effects so far
    /// left them.
    pub fn targets(&self) -> &CommandTargets {
        self.targets
    }

    /// The host world and every dimension world in the dimension order.
    pub fn host_and_worlds(&mut self) -> (&mut World, impl Iterator<Item = &mut World>) {
        (
            &mut *self.host,
            self.worlds.iter_mut().map(|world| &mut **world),
        )
    }

    /// The dimension world at `index` in the dimension order.
    pub fn world_mut(&mut self, index: usize) -> Option<&mut World> {
        self.worlds.get_mut(index).map(|world| &mut **world)
    }

    /// Fills the scratch packets from the world at `index`, then sends them in order.
    pub fn send(
        &mut self,
        index: usize,
        fill: impl FnOnce(&mut World, &mut Vec<OutboundPlayerPacket>),
    ) {
        let Some(world) = self.worlds.get_mut(index) else {
            return;
        };
        self.packets.clear();
        fill(world, self.packets);
        for packet in self.packets.drain(..) {
            deliver(self.host, packet);
        }
    }

    /// Sets the transform of the player with the host anchor `player` in the world that holds
    /// it. Returns the entity of the player there.
    pub fn set_transform(
        &mut self,
        player: Entity,
        transform: Transform,
    ) -> Result<Entity, PlayerChangeError> {
        let entry = self
            .targets
            .player_mut(player)
            .ok_or(PlayerChangeError::NotOnline)?;
        let mut current = self
            .worlds
            .get_mut(entry.dimension)
            .and_then(|world| world.get_mut::<Transform>(entry.entity))
            .ok_or(PlayerChangeError::NotInWorld)?;
        *current = transform;
        entry.transform = transform;
        Ok(entry.entity)
    }

    /// Moves the player with the host anchor `player` to the dimension at `destination`
    /// through [`change_dimension`]. Returns the entity of the player in its new world.
    pub fn change_dimension(
        &mut self,
        player: Entity,
        destination: usize,
        arrival: Arrival,
    ) -> Result<Entity, PlayerChangeError> {
        let entry = self
            .targets
            .player_mut(player)
            .ok_or(PlayerChangeError::NotOnline)?;
        let [from, to] = self
            .worlds
            .get_disjoint_mut([entry.dimension, destination])
            .map_err(|_| PlayerChangeError::NoDimension)?;
        let transform = arrival.transform;
        let arrived = change_dimension(self.host, from, to, entry.entity, arrival)
            .map_err(PlayerChangeError::DimensionChange)?;
        entry.dimension = destination;
        entry.entity = arrived;
        entry.transform = transform;
        Ok(arrived)
    }

    /// Sets the game mode of the player with the host anchor `player`. Its world sends the
    /// packets of the change on its next tick.
    pub fn set_game_mode(
        &mut self,
        player: Entity,
        mode: GameMode,
    ) -> Result<(), PlayerChangeError> {
        let entry = self
            .targets
            .player_mut(player)
            .ok_or(PlayerChangeError::NotOnline)?;
        let mut current = self
            .worlds
            .get_mut(entry.dimension)
            .and_then(|world| world.get_mut::<PlayerGameMode>(entry.entity))
            .ok_or(PlayerChangeError::NotInWorld)?;
        current.set_if_neq(PlayerGameMode(mode));
        entry.mode = mode;
        Ok(())
    }
}

impl Executor<SourceState, Handler> for SeamExecutor<'_, '_> {
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
        let modified: ModifyResult = self
            .call(invocation, |world| {
                world.get_resource::<ModifierHandlers>()?.get(key)
            })
            .ok_or_else(CommandFailure::failed)?;
        match modified? {
            Modified::None => {}
            Modified::One(source) => out.push(source),
            Modified::Many(sources) => out.extend(sources),
        }
        Ok(())
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
