use crate::command::effect::{CommandOutbox, Effect};
use crate::command::run::{CommandEvent, answer_suggestion, parse_line};
use crate::command::targets::{DimensionEntry, PlayerEntry, dimension_rank};
use crate::command::vanilla::{gamemode, teleport, time};
use crate::command::{
    CommandFailure, CommandHandlers, CommandLimits, CommandResult, CommandTargets, CommandTree,
    Handler, Invocation, Modified, ModifierHandlers, ModifyResult, RunningCommand, SourceState,
    system_message,
};
use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::PlayerGameMode;
use crate::world::entity::player::dimension_change::change_dimension;
use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{In, With};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::SystemId;
use bevy_ecs::world::World;
use mcrs_command::{End, Executor, Failure, SourceLists, Stage, execute};
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_engine::bridge::OutboundFlush;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::dim::deliver;
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::{Place, Session, SessionPlacement};
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeId};
use mcrs_minecraft_level::world::sub_app::DimensionLabel;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::text::{IntoText, Text};
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
    } = &mut seam;
    if let Some(mut received) = host.get_resource_mut::<CommandQueue>() {
        std::mem::swap(&mut *received, queue);
    }
    if let Some(mut owned) = host.get_resource_mut::<CommandTree>() {
        std::mem::swap(&mut *owned, tree);
    }
    let mut found: Vec<(DimensionEntry, &mut World)> = sub_apps
        .sub_apps
        .values_mut()
        .filter_map(|sub| {
            let world = sub.world_mut();
            let entry = dimension_entry(world)?;
            Some((entry, world))
        })
        .collect();
    found.sort_by(|(a, _), (b, _)| dimension_rank(&a.key).cmp(&dimension_rank(&b.key)));
    targets.dimensions.clear();
    let mut worlds: Vec<&mut World> = found
        .into_iter()
        .map(|(entry, world)| {
            targets.dimensions.push(entry);
            world
        })
        .collect();
    collect_players(host, &mut worlds, targets);
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

fn dimension_entry(world: &mut World) -> Option<DimensionEntry> {
    let label = world.get_resource::<DimensionLabel>()?.0;
    let (key, type_id) = world
        .query_filtered::<(
            &ResourceKey<mcrs_minecraft_dimension::Dimension>,
            &DimensionTypeId,
        ), With<Dimension>>()
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
/// player list. A player that leaves has no place, so no command finds it.
fn collect_players(host: &World, worlds: &mut [&mut World], targets: &mut CommandTargets) {
    targets.players.clear();
    for (dimension, world) in worlds.iter_mut().enumerate() {
        let Some(label) = targets.dimensions.get(dimension).map(|entry| entry.label) else {
            continue;
        };
        let mut players = world.query_filtered::<(
            Entity,
            &HostAnchor,
            &GameProfile,
            &Transform,
            &PlayerGameMode,
        ), With<Player>>();
        for (entity, &HostAnchor(anchor), profile, transform, mode) in players.iter(world) {
            let (Some(session), Some(placement)) = (
                host.get::<Session>(anchor),
                host.get::<SessionPlacement>(anchor),
            ) else {
                continue;
            };
            let placed = match placement.place() {
                Place::InDim(dim) | Place::Joining(dim) => dim == label,
                Place::Unplaced | Place::Transferring { .. } => false,
            };
            if placed {
                targets.players.push(PlayerEntry {
                    anchor,
                    dimension,
                    entity,
                    uuid: profile.id,
                    name: profile.username.clone(),
                    transform: *transform,
                    mode: mode.0,
                    list_order: session.0.0,
                });
            }
        }
    }
    targets.players.sort_by_key(|player| player.list_order);
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
        let Some((source, chain)) = self
            .in_world(index, |world| {
                world.run_system_cached_with(parse_line, &queued)
            })
            .and_then(|parsed| {
                parsed
                    .inspect_err(|error| tracing::error!("a command line did not parse: {error}"))
                    .ok()
            })
            .flatten()
        else {
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
        self.running.chain = chain;
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
        self.settle();
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

    /// Applies the effects of a run in order, then sends its messages.
    fn settle(&mut self) {
        let mut effects = std::mem::take(&mut self.outbox.effects);
        let moved = !effects.is_empty();
        let mut worlds = Worlds {
            host: self.host,
            worlds: self.worlds,
            targets: self.targets,
            packets: self.packets,
        };
        for effect in effects.drain(..) {
            match effect {
                Effect::GameMode { player, mode } => gamemode::apply(&mut worlds, player, mode),
                Effect::Teleport { player, to } => teleport::apply(&mut worlds, player, to),
                Effect::EditClock(edit) => time::apply(&mut worlds, edit),
            }
        }
        self.outbox.effects = effects;
        if moved {
            collect_players(self.host, self.worlds, self.targets);
        }
        for packet in self.outbox.packets.drain(..) {
            deliver(self.host, packet);
        }
        for broadcast in self.outbox.broadcasts.drain(..) {
            self.host.trigger(broadcast);
        }
    }
}

/// Every world, lent to the effects of one run so each applies where its target is.
pub(crate) struct Worlds<'s, 'w> {
    host: &'s mut World,
    worlds: &'s mut [&'w mut World],
    targets: &'s CommandTargets,
    packets: &'s mut Vec<OutboundPlayerPacket>,
}

impl Worlds<'_, '_> {
    pub(crate) fn host_and_worlds(&mut self) -> (&mut World, impl Iterator<Item = &mut World>) {
        (
            &mut *self.host,
            self.worlds.iter_mut().map(|world| &mut **world),
        )
    }

    pub(crate) fn dimension_of(&self, label: Entity) -> Option<usize> {
        self.targets.dimension_of(label)
    }

    pub(crate) fn world_mut(&mut self, index: usize) -> Option<&mut World> {
        self.worlds.get_mut(index).map(|world| &mut **world)
    }

    /// The index of the world that holds a player, and its entity there.
    pub(crate) fn locate(&mut self, player: Entity) -> Option<(usize, Entity)> {
        let (Place::InDim(label) | Place::Joining(label)) =
            self.host.get::<SessionPlacement>(player)?.place()
        else {
            return None;
        };
        let index = self.targets.dimension_of(label)?;
        let world = self.worlds.get_mut(index)?;
        let entity = world
            .query_filtered::<(Entity, &HostAnchor), With<Player>>()
            .iter(world)
            .find_map(|(entity, anchor)| (anchor.0 == player).then_some(entity))?;
        Some((index, entity))
    }

    pub(crate) fn transform_of(&mut self, player: Entity) -> Option<(usize, Entity, Transform)> {
        let (index, entity) = self.locate(player)?;
        let transform = *self.worlds.get(index)?.get::<Transform>(entity)?;
        Some((index, entity, transform))
    }

    /// Sets the transform of a player in its own world, or moves it to the world of
    /// `destination` (DIM-CHG-001), and gives its entity there.
    pub(crate) fn move_player(
        &mut self,
        player: Entity,
        origin: usize,
        destination: usize,
        entity: Entity,
        transform: Transform,
    ) -> Option<Entity> {
        if origin == destination {
            let world = self.worlds.get_mut(origin)?;
            *world.get_mut::<Transform>(entity)? = transform;
            return Some(entity);
        }
        let label = self.targets.dimensions.get(destination)?.label;
        // The packets already stamped for the player leave before its epoch changes, or the
        // bridge drops them as packets of the dimension it left.
        let _ = self.host.try_run_schedule(OutboundFlush);
        let [from, into] = self.worlds.get_disjoint_mut([origin, destination]).ok()?;
        let Some(arrived) = change_dimension(from, into, entity, transform) else {
            tracing::error!(?player, "a player did not change dimension");
            return None;
        };
        if let Some(mut placement) = self.host.get_mut::<SessionPlacement>(player) {
            placement.set(Place::InDim(label));
        }
        Some(arrived)
    }

    /// Fills the scratch packets from the world at `index`, then sends them in order.
    pub(crate) fn send(
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
