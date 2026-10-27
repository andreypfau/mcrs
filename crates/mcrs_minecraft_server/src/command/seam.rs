use crate::command::effect::{CommandOutbox, Effect, Facing, Teleport};
use crate::command::run::CommandEvent;
use crate::command::targets::{DimensionEntry, PlayerEntry, dimension_rank};
use crate::command::{
    CommandFailure, CommandHandlers, CommandLimits, CommandResult, CommandTargets, Handler,
    Invocation, Modified, ModifierHandlers, ModifyResult, RunningCommand, SourceState,
    system_message,
};
use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::PlayerGameMode;
use crate::world::entity::player::dimension_change::{arrival_packets, change_dimension};
use crate::world::entity::player::movement::TeleportState;
use crate::world::entity::player_network_id;
use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{DetectChangesMut, In, With};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::SystemId;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_command::{Chain, End, Executor, Failure, SourceLists, Stage, execute};
use mcrs_minecraft_command::anchored;
use mcrs_minecraft_command::teleport::{Pose, look_at, relative_move};
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_engine::bridge::OutboundFlush;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, to};
use mcrs_minecraft_engine::dim::deliver;
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_entity::PLAYER_EYE_HEIGHT;
use mcrs_minecraft_level::entity::physics::{Rotation, Transform};
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::{Place, Session, SessionPlacement};
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeId};
use mcrs_minecraft_level::world::sub_app::DimensionLabel;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundPlayerLookAt, ClientboundPlayerPosition,
};
use mcrs_minecraft_protocol::text::{IntoText, Text};
use mcrs_minecraft_protocol::{PositionFlags, VarInt};
use mcrs_minecraft_registries::CurrentRegistries;

/// The command lines this dimension parsed since the host last ran them.
#[derive(Resource, Default)]
pub(crate) struct PendingLines(pub(crate) Vec<PendingLine>);

pub(crate) struct PendingLine {
    /// The host anchor of the player who receives the messages of the line.
    pub(crate) output: Entity,
    pub(crate) event: CommandEvent,
    pub(crate) chain: Chain<Handler>,
}

/// The host's buffers for running command lines, reused from one run to the next.
#[derive(Resource, Default)]
pub(crate) struct CommandSeam {
    targets: CommandTargets,
    running: RunningCommand,
    outbox: CommandOutbox,
    stages: Vec<Stage<Handler>>,
    lists: SourceLists<SourceState>,
    lines: Vec<PendingLine>,
    packets: Vec<OutboundPlayerPacket>,
}

/// Runs the command lines that every dimension parsed, one after another, with every
/// dimension world in reach. The server calls it between two ticks, after the dimensions hand
/// over their packets; an embedder that drives the app itself calls it at the same point.
pub fn run_pending_commands(app: &mut App) {
    let sub_apps = app.sub_apps_mut();
    let waiting = sub_apps.sub_apps.values().any(|sub| {
        sub.world()
            .get_resource::<PendingLines>()
            .is_some_and(|pending| !pending.0.is_empty())
    });
    if !waiting {
        return;
    }
    let host = sub_apps.main.world_mut();
    let Some(mut seam) = host.remove_resource::<CommandSeam>() else {
        return;
    };
    let limits = host
        .get_resource::<CommandLimits>()
        .copied()
        .unwrap_or_default()
        .0;
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
    let CommandSeam {
        targets,
        running,
        outbox,
        stages,
        lists,
        lines,
        packets,
    } = &mut seam;
    targets.dimensions.clear();
    let mut worlds: Vec<&mut World> = found
        .into_iter()
        .map(|(entry, world)| {
            targets.dimensions.push(entry);
            world
        })
        .collect();
    collect_players(host, &mut worlds, targets);
    for world in &mut worlds {
        if let Some(mut pending) = world.get_resource_mut::<PendingLines>() {
            lines.append(&mut pending.0);
        }
    }
    let mut lines_now = std::mem::take(lines);
    for line in lines_now.drain(..) {
        let mut executor = SeamExecutor {
            host: &mut *host,
            worlds: &mut worlds,
            targets: &mut *targets,
            running: &mut *running,
            outbox: &mut *outbox,
            packets: &mut *packets,
            output: line.output,
        };
        executor.run_line(line, stages, lists, limits);
    }
    *lines = lines_now;
    targets.players.clear();
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

/// The run of one command line. Each stage call runs in the world of the dimension of its
/// source, with the line, the targets and the outbox lent to that world; what the call leaves
/// in the outbox applies before the next call.
struct SeamExecutor<'a, 'w> {
    host: &'a mut World,
    worlds: &'a mut [&'w mut World],
    targets: &'a mut CommandTargets,
    running: &'a mut RunningCommand,
    outbox: &'a mut CommandOutbox,
    packets: &'a mut Vec<OutboundPlayerPacket>,
    output: Entity,
}

/// Swaps the values of the host and of `world`; a second swap gives them back.
fn lend(
    world: &mut World,
    targets: &mut CommandTargets,
    running: &mut RunningCommand,
    outbox: &mut CommandOutbox,
) -> bool {
    let present = world.contains_resource::<CommandTargets>()
        && world.contains_resource::<RunningCommand>()
        && world.contains_resource::<CommandOutbox>();
    if present {
        std::mem::swap(&mut *world.resource_mut::<CommandTargets>(), targets);
        std::mem::swap(&mut *world.resource_mut::<RunningCommand>(), running);
        std::mem::swap(&mut *world.resource_mut::<CommandOutbox>(), outbox);
    }
    present
}

impl SeamExecutor<'_, '_> {
    fn run_line(
        &mut self,
        line: PendingLine,
        stages: &mut Vec<Stage<Handler>>,
        lists: &mut SourceLists<SourceState>,
        limits: mcrs_command::Limits,
    ) {
        let PendingLine {
            mut event, chain, ..
        } = line;
        let Some(world) = self
            .targets
            .dimension_of(event.source.dimension)
            .and_then(|index| self.worlds.get_mut(index))
        else {
            return;
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

    fn invocation(&self, stage: usize, handler: &Handler, source: &SourceState) -> Invocation {
        Invocation {
            output: self.output,
            source: source.clone(),
            stage,
            branch: handler.branch,
        }
    }

    /// Runs the system `pick` finds in the world of the source dimension, then settles what
    /// the run left. `None` when that world has no such system or the system did not run.
    fn call<O: 'static>(
        &mut self,
        invocation: Invocation,
        pick: impl FnOnce(&World) -> Option<SystemId<In<Invocation>, O>>,
    ) -> Option<O> {
        let world = self
            .targets
            .dimension_of(invocation.source.dimension)
            .and_then(|index| self.worlds.get_mut(index))?;
        let system = pick(world)?;
        if !lend(world, self.targets, self.running, self.outbox) {
            return None;
        }
        let result = world.run_system_with(system, invocation);
        lend(world, self.targets, self.running, self.outbox);
        self.settle();
        result
            .inspect_err(|error| tracing::error!("a command handler did not run: {error}"))
            .ok()
    }

    /// Applies the effects of a run in order, then sends its messages.
    fn settle(&mut self) {
        let mut effects = std::mem::take(&mut self.outbox.effects);
        for effect in effects.drain(..) {
            match effect {
                Effect::GameMode { player, mode } => self.set_game_mode(player, mode),
                Effect::Teleport { player, to } => self.teleport(player, to),
            }
        }
        self.outbox.effects = effects;
        for packet in self.outbox.packets.drain(..) {
            deliver(self.host, packet);
        }
        for broadcast in self.outbox.broadcasts.drain(..) {
            self.host.trigger(broadcast);
        }
    }

    fn set_game_mode(&mut self, player: Entity, mode: mcrs_minecraft_protocol::GameMode) {
        let Some(entry) = self.targets.player_mut(player) else {
            tracing::error!(
                ?player,
                "a game mode change names no player a command found"
            );
            return;
        };
        if let Some(mut current) = self
            .worlds
            .get_mut(entry.dimension)
            .and_then(|world| world.get_mut::<PlayerGameMode>(entry.entity))
        {
            current.set_if_neq(PlayerGameMode(mode));
            entry.mode = mode;
        }
    }

    /// The position a victim turns to, and the entity of `player_look_at` (CMD-TP-011,
    /// CMD-TP-012), read after the moves before it.
    fn facing(&self, facing: Facing) -> Option<(DVec3, Option<(VarInt, LookAnchorKind)>)> {
        match facing {
            Facing::Position(position) => Some((position, None)),
            Facing::Player { anchor, at } => self.targets.player(anchor).map(|player| {
                (
                    anchored(at, player.transform.translation, PLAYER_EYE_HEIGHT),
                    Some((VarInt(player_network_id(anchor)), at)),
                )
            }),
        }
    }

    /// Moves one victim (CMD-TP-005, CMD-TP-013); to another dimension as DIM-CHG states.
    fn teleport(&mut self, player: Entity, to: Teleport) {
        let (Some(victim), Some(destination)) = (
            self.targets.player(player),
            self.targets.dimension_of(to.dimension),
        ) else {
            tracing::error!(
                ?player,
                "a teleport names no player or dimension a command found"
            );
            return;
        };
        let (origin, entity, current) = (victim.dimension, victim.entity, victim.transform);
        let from = Pose {
            position: current.translation,
            yaw: current.rotation.yaw(),
            pitch: current.rotation.pitch(),
        };
        let (yaw, pitch) = to.rotation.unwrap_or((from.yaw, from.pitch));
        let destination_pose = Pose {
            position: to.position,
            yaw,
            pitch,
        };
        let step = relative_move(
            from,
            destination_pose,
            to.relative_position,
            to.relative_rotation,
            origin != destination,
        );
        let mut transform = Transform {
            translation: to.position,
            rotation: Rotation::new(step.result.yaw, step.result.pitch),
        };
        self.place(player, destination, entity, transform);
        let facing = to.facing.and_then(|facing| self.facing(facing));
        if let Some((target, _)) = facing {
            let (yaw, pitch) = look_at(anchored(to.anchor, to.position, PLAYER_EYE_HEIGHT), target);
            transform.rotation = Rotation::new(yaw, pitch);
        }
        let Some(arrived) = self.move_entity(player, origin, destination, entity, transform) else {
            return;
        };
        self.place(player, destination, arrived, transform);
        let Some(world) = self.worlds.get_mut(destination) else {
            return;
        };
        let Some(mut state) = world.get_mut::<TeleportState>(arrived) else {
            return;
        };
        let position = ClientboundPlayerPosition {
            teleport_id: VarInt(state.sent(transform)),
            position: step.position,
            velocity: DVec3::ZERO,
            look: step.look,
            flags: PositionFlags(step.flags),
        };
        self.packets.clear();
        if origin == destination {
            self.packets
                .push(to_player(player, PacketPayload::PlayerPosition(position)));
        } else {
            arrival_packets(world, arrived, position, self.packets);
        }
        if let Some((target, entity)) = facing {
            self.packets.push(to_player(
                player,
                PacketPayload::PlayerLookAt(ClientboundPlayerLookAt {
                    from: to.anchor,
                    target,
                    entity,
                }),
            ));
        }
        for packet in self.packets.drain(..) {
            deliver(self.host, packet);
        }
    }

    fn place(&mut self, player: Entity, dimension: usize, entity: Entity, transform: Transform) {
        if let Some(entry) = self.targets.player_mut(player) {
            entry.dimension = dimension;
            entry.entity = entity;
            entry.transform = transform;
        }
    }

    /// Sets the transform of a victim in its own world, or moves it to the world of
    /// `destination`, and gives its entity there.
    fn move_entity(
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
}

type LookAnchorKind = mcrs_minecraft_protocol::packets::game::clientbound::LookAnchor;

fn to_player(player: Entity, payload: PacketPayload) -> OutboundPlayerPacket {
    to(player, payload).critical()
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
