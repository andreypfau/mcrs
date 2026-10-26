use crate::command::{
    CommandContext, CommandFailure, CommandHandlers, CommandLimits, CommandTree, Departures,
    ExecScratch, Handler, Invocation, Modified, ModifierHandlers, RunningCommand, SourceState,
    level_of,
};
use crate::ops::OpList;
use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::{PlayerGameMode, PlayerOpLevel};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_math::DVec3;
use mcrs_command::{
    Chain, End, Executor, Failure, ParseScratch, Resume, Suggestion, build_list, resume,
};
use mcrs_minecraft_command::{
    ArgumentError, CommandLevel, NoLookup, Source, error_feedback, player_tree, suggestion_reply,
};
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_engine::bus::{CommandHandoff, HandoffSource};
use mcrs_minecraft_engine::bus::{
    OperatorBroadcast, OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget, to,
};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::{PlayerSession, Session};
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeId};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChatCommand, ServerboundCommandSuggestion,
};
use mcrs_minecraft_protocol::text::{Color, IntoText, Text, is_allowed_chat_character};
use mcrs_minecraft_protocol::{GameMode, VarInt};
use mcrs_minecraft_registries::CurrentRegistries;
use smallvec::SmallVec;

/// The text any player may ask suggestions for; a player who can edit command blocks may send
/// up to the packet's own limit (CMDP-SUG-001).
const CHAT_SUGGESTION_UNITS: usize = 256;

pub(crate) fn system_message(host: Entity, content: Text) -> OutboundPlayerPacket {
    to(
        host,
        PacketPayload::SystemChat(ClientboundSystemChatPacket {
            content,
            overlay: false,
        }),
    )
}

/// The host disconnects a player whose command line holds a code unit no player can type
/// (CMDP-EXEC-002); the dimension drops the line.
pub(crate) fn reject_illegal_characters(
    event: On<Received<ServerboundChatCommand<'static>>>,
    mut connections: Query<&mut ServerSideConnection>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if packet.command.0.chars().all(is_allowed_chat_character) {
        return;
    }
    if let Ok(mut connection) = connections.get_mut(event.entity) {
        connection
            .disconnect(
                ConnectionState::Game,
                Text::translate("multiplayer.disconnect.illegal_characters", vec![]),
            )
            .ok();
        commands
            .entity(event.entity)
            .remove::<ServerSideConnection>();
    }
}

/// Raised before a command line runs. When an observer sets `cancelled`, the command does not
/// run and the source receives nothing.
#[derive(Event, Debug)]
pub struct CommandEvent {
    player: Entity,
    source: SourceState,
    line: String,
    pub cancelled: bool,
}

/// Read-only: the parse of the chain belongs to this line and this source.
impl CommandEvent {
    pub fn player(&self) -> Entity {
        self.player
    }

    pub fn source(&self) -> &SourceState {
        &self.source
    }

    pub fn line(&self) -> &str {
        &self.line
    }
}

pub(crate) fn handle_chat_command(
    event: On<Received<ServerboundChatCommand<'static>>>,
    context: CommandContext,
    players: Query<(&PlayerOpLevel, &Transform, &HostAnchor), With<Player>>,
    mut scratch: Local<ParseScratch<ArgumentError>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
    mut commands: Commands,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let line = packet.command.0;
    let Ok((&op_level, transform, &HostAnchor(host))) = players.get(event.entity) else {
        return;
    };
    if !line.chars().all(is_allowed_chat_character) {
        return;
    }
    let state = SourceState::of_player(event.entity, level_of(op_level), transform);
    let source = state.source(&context);
    let tree = &context.tree;
    let parse = tree.0.parse(line, 0, &source, &mut scratch);
    if let Err(error) = parse.check(&tree.0) {
        for message in error_feedback(line, &error) {
            writer.write(system_message(host, message));
        }
        return;
    }
    let mut chain = Chain::default();
    chain.copy_from(&tree.0, &parse);
    let event = CommandEvent {
        player: event.entity,
        source: state,
        line: line.to_owned(),
        cancelled: false,
    };
    commands.queue(move |world: &mut World| run_line(world, host, event, chain));
}

/// Runs a command line that parsed: the seam where the chain drives the handler systems.
fn run_line(world: &mut World, host: Entity, mut event: CommandEvent, chain: Chain<Handler>) {
    world.trigger_ref(&mut event);
    if event.cancelled {
        return;
    }
    let CommandEvent { source, line, .. } = event;
    let limits = world
        .get_resource::<CommandLimits>()
        .copied()
        .unwrap_or_default()
        .0;
    run_chain(world, host, line, chain, Resume::start(limits), [source]);
}

pub(crate) fn has_handed_off_lines(handoffs: Option<Res<Messages<CommandHandoff>>>) -> bool {
    handoffs.is_some_and(|handoffs| !handoffs.is_empty())
}

/// Queues the rest of each command line whose sources arrived from another dimension.
pub(crate) fn resume_handed_off_lines(
    mut handoffs: ResMut<Messages<CommandHandoff>>,
    mut commands: Commands,
) {
    for handoff in handoffs.drain() {
        commands.queue(move |world: &mut World| resume_line(world, handoff));
    }
}

/// The coordinate scale of the dimension of this world.
fn coordinate_scale(world: &mut World) -> Option<f64> {
    let type_id = world
        .query_filtered::<&DimensionTypeId, With<Dimension>>()
        .single(world)
        .ok()?
        .0;
    let types = world
        .get_resource::<CurrentRegistries>()?
        .registry::<DimensionTypeFile>()?;
    let id = mcrs_minecraft_core::Id::checked(type_id.number(), types.len())?;
    types.get(id).map(|file| file.coordinate_scale)
}

fn fail_line(world: &mut World, output: Entity, forked: bool) {
    if !forked {
        world.write_message(system_message(output, CommandFailure::failed().text()));
    }
}

fn resume_line(world: &mut World, handoff: CommandHandoff) {
    let CommandHandoff {
        output,
        line,
        coordinate_scale: from,
        rest,
        ..
    } = handoff;
    let forked = rest.at.forked;
    let (Some(here), Some(tree)) = (coordinate_scale(world), world.get_resource::<CommandTree>())
    else {
        fail_line(world, output, forked);
        return;
    };
    let (chain, at, sources) = rest.into_parts(&tree.0);
    let scale = from / here;
    // chisle: an arriving source has no entity, so `@s` finds nothing and the admin echo names
    // nobody; an entity index across the dimension worlds lifts it.
    let sources = sources.into_iter().map(|source| SourceState {
        level: source.level,
        entity: None,
        position: DVec3::new(
            source.position.x * scale,
            source.position.y,
            source.position.z * scale,
        ),
        yaw: source.yaw,
        pitch: source.pitch,
        anchor: source.anchor,
        destination: None,
    });
    run_chain(world, output, line, chain, at, sources);
}

/// Runs a chain from `at` and hands the sources that leave this world to the host.
fn run_chain(
    world: &mut World,
    output: Entity,
    line: String,
    chain: Chain<Handler>,
    at: Resume,
    sources: impl IntoIterator<Item = SourceState>,
) {
    let mut scratch = world
        .get_resource_mut::<ExecScratch>()
        .map(|mut scratch| std::mem::take(&mut *scratch))
        .unwrap_or_default();
    scratch.stages.clear();
    scratch.stages.extend_from_slice(chain.stages());
    scratch.departing.clear();
    let Some(mut running) = world.get_resource_mut::<RunningCommand>() else {
        return;
    };
    running.line = line;
    running.chain = chain;
    let mut executor = WorldExecutor {
        world,
        output,
        departing: &mut scratch.departing,
    };
    let end = resume(
        &scratch.stages,
        at,
        sources,
        &mut executor,
        &mut scratch.lists,
    );
    if end == End::SequenceLimit {
        tracing::info!(
            "a command stopped at the limit of {} stages and runs",
            at.limits.sequence.max(1)
        );
    }
    hand_off(world, output, &mut scratch.departing);
    world.insert_resource(scratch);
}

/// Sends each part of the sources that left to the host, which passes it to its dimension.
/// The last part moves the chain; the others copy it.
fn hand_off(world: &mut World, output: Entity, departing: &mut Vec<Departing>) {
    if departing.is_empty() {
        return;
    }
    let forked = departing.iter().all(|part| part.at.forked);
    let running = world
        .get_resource_mut::<RunningCommand>()
        .map(|mut running| std::mem::take(&mut *running));
    let (Some(scale), Some(mut running)) = (coordinate_scale(world), running) else {
        departing.clear();
        fail_line(world, output, forked);
        return;
    };
    let Some(mut departures) = world.get_resource_mut::<Departures>() else {
        departing.clear();
        fail_line(world, output, forked);
        return;
    };
    let mut parts = departing.drain(..).peekable();
    while let Some(part) = parts.next() {
        let (line, rest) = if parts.peek().is_some() {
            let rest = running.chain.continuation(part.at, part.sources);
            (running.line.clone(), rest)
        } else {
            let chain = std::mem::take(&mut running.chain);
            let rest = chain.into_continuation(part.at, part.sources);
            (std::mem::take(&mut running.line), rest)
        };
        departures.0.push(CommandHandoff {
            target: part.target,
            output,
            line,
            coordinate_scale: scale,
            rest,
        });
    }
}

/// The sources of one part of a list that leave for `target`.
pub(crate) struct Departing {
    target: ResourceKey<mcrs_minecraft_dimension::Dimension>,
    at: Resume,
    sources: Vec<HandoffSource>,
}

struct WorldExecutor<'w, 'd> {
    world: &'w mut World,
    output: Entity,
    departing: &'d mut Vec<Departing>,
}

impl WorldExecutor<'_, '_> {
    fn invocation(&self, stage: usize, handler: &Handler, source: &SourceState) -> Invocation {
        Invocation {
            output: self.output,
            source: source.clone(),
            stage,
            branch: handler.branch,
        }
    }
}

impl Executor<SourceState, Handler> for WorldExecutor<'_, '_> {
    type Error = CommandFailure;

    fn modify(
        &mut self,
        stage: usize,
        modifier: &Handler,
        source: &SourceState,
        out: &mut Vec<SourceState>,
    ) -> Result<(), CommandFailure> {
        let system = self
            .world
            .get_resource::<ModifierHandlers>()
            .and_then(|handlers| handlers.get(modifier.key))
            .ok_or_else(CommandFailure::failed)?;
        let invocation = self.invocation(stage, modifier, source);
        match self.world.run_system_with(system, invocation) {
            Ok(Ok(Modified::None)) => {}
            Ok(Ok(Modified::One(source))) => out.push(source),
            Ok(Ok(Modified::Many(sources))) => out.extend(sources),
            Ok(Err(failure)) => return Err(failure),
            Err(error) => {
                tracing::error!("a command modifier did not run: {error}");
                return Err(CommandFailure::failed());
            }
        }
        Ok(())
    }

    fn run(
        &mut self,
        stage: usize,
        command: &Handler,
        source: &SourceState,
    ) -> Result<i32, CommandFailure> {
        let system = self
            .world
            .get_resource::<CommandHandlers>()
            .and_then(|handlers| handlers.get(command.key))
            .ok_or_else(CommandFailure::failed)?;
        let invocation = self.invocation(stage, command, source);
        self.world
            .run_system_with(system, invocation)
            .unwrap_or_else(|error| {
                tracing::error!("a command handler did not run: {error}");
                Err(CommandFailure::failed())
            })
    }

    fn report(&mut self, failure: Failure<CommandFailure>) {
        let failure = match failure {
            Failure::Command(failure) => failure,
            Failure::ForkLimit(limit) => {
                CommandFailure::translate("command.forkLimit", vec![limit.to_string().into_text()])
            }
        };
        self.world
            .write_message(system_message(self.output, failure.text()));
    }

    fn departs(&mut self, source: &SourceState) -> bool {
        source.destination.is_some()
    }

    /// One part for each destination, in the order the destinations first appear.
    fn depart(&mut self, at: Resume, total: usize, sources: &mut Vec<SourceState>) {
        let first = self.departing.len();
        for source in sources.drain(..) {
            let Some(target) = source.destination else {
                continue;
            };
            let carried = HandoffSource {
                level: source.level,
                position: source.position,
                yaw: source.yaw,
                pitch: source.pitch,
                anchor: source.anchor,
            };
            let parts = self.departing.get_mut(first..).unwrap_or_default();
            match parts.iter_mut().find(|part| part.target == target) {
                Some(part) => part.sources.push(carried),
                None => self.departing.push(Departing {
                    target,
                    at,
                    sources: vec![carried],
                }),
            }
        }
        if let Some(parts) = self.departing.get_mut(first..) {
            for part in parts {
                part.at = at.part(total, part.sources.len());
            }
        }
    }
}

/// The latest suggestion request of a player that has no reply yet (CMDP-SUG-003). A newer
/// request replaces it, and the replaced one gets no reply.
#[derive(Component, Debug, Default)]
pub(crate) struct PendingSuggestion(Option<(VarInt, String)>);

pub(crate) fn queue_suggestion_request(
    event: On<Received<ServerboundCommandSuggestion<'static>>>,
    mut players: Query<(&PlayerOpLevel, &PlayerGameMode, &mut PendingSuggestion), With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Ok((&op_level, mode, mut pending)) = players.get_mut(event.entity) else {
        return;
    };
    let text = packet.text.0;
    let edits_command_blocks =
        mode.0 == GameMode::Creative && level_of(op_level) >= CommandLevel::Gamemasters;
    if !edits_command_blocks && text.encode_utf16().count() > CHAT_SUGGESTION_UNITS {
        return;
    }
    match &mut pending.0 {
        Some((id, stored)) => {
            *id = packet.id;
            stored.clear();
            stored.push_str(text);
        }
        slot @ None => *slot = Some((packet.id, text.to_owned())),
    }
}

pub(crate) fn has_pending_suggestion(
    pending: Query<&PendingSuggestion, Changed<PendingSuggestion>>,
) -> bool {
    pending.iter().any(|pending| pending.0.is_some())
}

/// Moves the items out of `buffer` into a buffer of another lifetime, keeping its allocation:
/// the two element types differ only in lifetimes, so the collect reuses the buffer in place.
fn recycle<A, B>(buffer: Vec<A>) -> Vec<B> {
    buffer.into_iter().filter_map(|_| None).collect()
}

#[derive(Default)]
pub(crate) struct SuggestionScratch {
    parse: ParseScratch<ArgumentError>,
    suggestions: Vec<Suggestion<'static, &'static str>>,
}

/// Answers the pending request of each player, one per player per tick, then clears it.
pub(crate) fn answer_suggestion_requests(
    context: CommandContext,
    mut players: Query<
        (
            Entity,
            &PlayerOpLevel,
            &Transform,
            &HostAnchor,
            &mut PendingSuggestion,
        ),
        (With<Player>, Changed<PendingSuggestion>),
    >,
    mut scratch: Local<SuggestionScratch>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let scratch = &mut *scratch;
    for (entity, &op_level, transform, &HostAnchor(host), mut pending) in &mut players {
        let Some((id, text)) = pending.0.as_ref() else {
            continue;
        };
        let state = SourceState::of_player(entity, level_of(op_level), transform);
        let source = state.source(&context);
        let start = usize::from(text.starts_with('/'));
        let parse = context
            .tree
            .0
            .parse(text, start, &source, &mut scratch.parse);
        let mut suggestions = recycle(std::mem::take(&mut scratch.suggestions));
        context.tree.0.suggest(&parse, &source, &mut suggestions);
        let list = build_list(text, &mut suggestions);
        writer.write(to(
            host,
            PacketPayload::CommandSuggestions(suggestion_reply(*id, text, &list)),
        ));
        scratch.suggestions = recycle(suggestions);
        pending.0 = None;
    }
}

/// Sends a player the tree it can see whenever its level is set, which includes its arrival
/// in the dimension (CMDP-TREE-002).
pub(crate) fn send_command_tree(
    tree: Res<CommandTree>,
    players: Query<(&PlayerOpLevel, &HostAnchor), Changed<PlayerOpLevel>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (&op_level, &HostAnchor(host)) in &players {
        let source = Source::new(level_of(op_level), &NoLookup);
        writer.write(to(host, PacketPayload::Commands(player_tree(&tree.0, &source))).critical());
    }
}

/// Sends what a command says to its source and to the operators who watch it
/// (CMDP-EXEC-004).
#[derive(SystemParam)]
pub struct CommandFeedback<'w, 's> {
    writer: MessageWriter<'w, OutboundPlayerPacket>,
    broadcasts: MessageWriter<'w, OperatorBroadcast>,
    players: Query<'w, 's, (&'static HostAnchor, &'static GameProfile)>,
}

impl CommandFeedback<'_, '_> {
    /// A packet the command sends besides its messages.
    pub fn send(&mut self, packet: OutboundPlayerPacket) {
        self.writer.write(packet);
    }

    pub fn tell(&mut self, player: Entity, message: Text) {
        if let Ok((&HostAnchor(host), _)) = self.players.get(player) {
            self.writer.write(system_message(host, message));
        }
    }

    /// A success message; with `broadcast`, every other operator sees it too.
    pub fn success(&mut self, invocation: &Invocation, message: Text, broadcast: bool) {
        // chisle: `send_command_feedback` and `log_admin_commands` have no state in the server
        // yet, so both hold their default true; a game rule resource lifts it.
        self.writer
            .write(system_message(invocation.output, message.clone()));
        if !broadcast {
            return;
        }
        let Some(Ok((_, profile))) = invocation
            .source
            .entity
            .map(|entity| self.players.get(entity))
        else {
            return;
        };
        // The server has no language table, so the log keeps the translation key and its
        // arguments as JSON.
        tracing::info!("[{}: {message}]", profile.username);
        let admin = Text::translate(
            "chat.type.admin",
            vec![profile.username.clone().into_text(), message],
        )
        .color(Color::GRAY)
        .italic();
        self.broadcasts.write(OperatorBroadcast {
            source: invocation.output,
            message: admin,
        });
    }
}

/// Delivers an admin message from any dimension to every online operator but its source. The
/// host holds each session's profile and the operator list, so the set is read here when a
/// message arrives.
pub(crate) fn broadcast_to_operators(
    broadcast: On<OperatorBroadcast>,
    sessions: Query<(Entity, &GameProfile), With<Session>>,
    ops: Option<Res<OpList>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Some(ops) = ops else {
        return;
    };
    let operators: SmallVec<[Entity; 8]> = sessions
        .iter()
        .filter(|&(anchor, profile)| anchor != broadcast.source && ops.get(&profile.id).is_some())
        .map(|(anchor, _)| anchor)
        .collect();
    if operators.is_empty() {
        return;
    }
    writer.write(OutboundPlayerPacket {
        target: PacketTarget::PlayerSet(operators),
        priority: PacketPriority::Normal,
        data: PacketPayload::SystemChat(ClientboundSystemChatPacket {
            content: broadcast.message.clone(),
            overlay: false,
        }),
        session: PlayerSession(0),
        epoch: 0,
    });
}
