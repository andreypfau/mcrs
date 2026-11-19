use crate::menu_view::MenuView;
use crate::pose::Pose;
use crate::swing::{CapturedSwing, EntityNames};
use crate::{
    AnimateObserved, Click, CommandTree, ConnectionState, MenuPacket, Observe, PacketKind,
    PredictedStack, SoundPlayed, SwingObserved,
};
use anyhow::Context;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_network::SwitchMode;
use mcrs_minecraft_network::join::Configuration;
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::common::serverbound::{ClientInformation, KeepAlive};
use mcrs_minecraft_protocol::packets::configuration::clientbound::ClientboundDisconnect as ConfigurationDisconnect;
use mcrs_minecraft_protocol::packets::configuration::serverbound::{
    ServerboundClientInformation as ConfigClientInformation, ServerboundFinishConfiguration,
};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundDisconnect, ClientboundKeepAlive,
};
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundKeepAlive;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::login::clientbound::{
    ClientboundLoginDisconnect, ClientboundLoginFinished, ClientboundLoginFinishedWithOptions,
    LoginCompression, OptionFlags,
};
use mcrs_minecraft_protocol::packets::login::serverbound::{
    ServerboundHello, ServerboundLoginAcknowledged,
};
use mcrs_minecraft_protocol::setting::{ChatMode, DisplayedSkinParts, MainArm, ParticleStatus};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{
    Bounded, CompressionThreshold, Decode, Encode, Packet, VarInt, WritePacket,
};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundBlockChangedAck, ClientboundBlockUpdate,
    ClientboundChunkBatchFinished, ClientboundCommandSuggestions, ClientboundCommands,
    ClientboundEntityPositionSync, ClientboundLogin, ClientboundMoveEntityPos,
    ClientboundMoveEntityPosRot, ClientboundPlayerPosition, ClientboundRemoveEntities,
    ClientboundRespawn, ClientboundSectionBlocksUpdate, ClientboundSetEntityData,
    ClientboundSetEquipment, ClientboundSystemChatPacket, ClientboundTeleportEntity,
};
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAttack, ServerboundChatCommand, ServerboundChunkBatchReceived,
    ServerboundClientInformation as GameClientInformation, ServerboundCommandSuggestion,
    ServerboundContainerClick, ServerboundContainerClose, ServerboundInteract,
    ServerboundPlayerAction, ServerboundPlayerLoaded, ServerboundPunch, ServerboundSetCarriedItem,
    ServerboundSetCreativeModeSlot, ServerboundUseItem, ServerboundUseItemOn,
};
use mcrs_minecraft_protocol_game::{Direction, Hand, LpVec3, Position};
use mcrs_minecraft_text::Text;
use mcrs_voxel::BlockPos;
use std::collections::VecDeque;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::ops::ControlFlow;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::time;

const CLOSE_LIMIT: Duration = Duration::from_secs(5);
pub(crate) const WRITE_LIMIT: Duration = Duration::from_secs(5);
const CHUNKS_PER_TICK: f32 = 64.0;

pub(crate) struct Joined {
    pub(crate) io: Box<PacketIo<TcpStream>>,
    pub(crate) player_id: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoadSignal {
    Send,
    Defer,
}

pub(crate) enum Login {
    Joined(Joined),
    Refused {
        state: ConnectionState,
        reason: Option<Text>,
    },
}

pub(crate) struct QueuedOrder {
    pub(crate) order: Order,
    pub(crate) deadline: time::Instant,
    pub(crate) written: Option<oneshot::Sender<Result<(), String>>>,
}

pub(crate) enum Order {
    Command(String),
    Suggest {
        id: i32,
        line: String,
    },
    ClientMainArm(MainArm),
    PunchExtraByte,
    Leave,
    SendLoaded,
    SyncPosition,
    Click(Click),
    Close(Option<i32>),
    SelectSlot(i16),
    PlayerAction {
        action: PlayerAction,
        position: [i32; 3],
        face: Direction,
    },
    CreativeSlot {
        slot: i16,
        stack: Option<PredictedStack>,
    },
    UseBlock([i32; 3]),
    Punch,
    PunchBurst(u32),
    UseItem {
        hand: Hand,
        sequence: i32,
    },
    UseItemOn {
        hand: Hand,
        position: [i32; 3],
        face: Direction,
        sequence: i32,
    },
    Attack(String),
    Interact {
        target: String,
        hand: Hand,
        secondary: bool,
    },
}

pub(crate) struct Event {
    pub(crate) client: String,
    pub(crate) signal: Signal,
    pub(crate) at: time::Instant,
}

impl Event {
    pub(crate) fn now(client: String, signal: Signal) -> Self {
        Self {
            client,
            signal,
            at: time::Instant::now(),
        }
    }
}

pub(crate) enum Signal {
    Chat(Text),
    Menu(MenuPacket),
    Sound(SoundPlayed),
    Commands(CommandTree),
    Suggestions {
        id: i32,
        start: i32,
        length: i32,
        suggestions: Vec<String>,
    },
    Swing(CapturedSwing),
    BlockAck(i32),
    Added(String),
    Removed(Vec<String>),
    PositionAccepted(Position),
    Animate(AnimateObserved),
    Packet(PacketKind),
    Disconnected(Text),
    Closed,
    Failed(String),
}

pub(crate) async fn log_in(address: SocketAddr, name: &str) -> anyhow::Result<Login> {
    let mut io = PacketIo::connect(address).await?;
    let switch_mode = SwitchMode::of_own_version();
    io.send_packet(&ServerboundHandshake {
        protocol_version: VarInt(VERSION.protocol_version),
        server_address: Bounded(address.ip().to_string().as_str()),
        server_port: address.port(),
        intent: Intent::Login,
    })
    .await?;
    io.send_packet(&ServerboundHello {
        username: Bounded(name),
        profile_id: Uuid::nil(),
    })
    .await?;

    let mut state = ConnectionState::Login;
    let mut stored_threshold = CompressionThreshold(0);
    let mut configuration = Configuration::default();
    loop {
        let (id, body) = match io.recv_frame().await {
            Ok(frame) => frame,
            Err(error) if is_close(&error) => {
                return Ok(Login::Refused {
                    state,
                    reason: None,
                });
            }
            Err(error) => return Err(error),
        };
        let reason = match state {
            ConnectionState::Login if id == ClientboundLoginDisconnect::ID => {
                let packet: ClientboundLoginDisconnect = decode(&body)?;
                Some(serde_json::from_str(packet.reason.0)?)
            }
            ConnectionState::Login if id == LoginCompression::ID => {
                let packet: LoginCompression = decode(&body)?;
                let threshold = CompressionThreshold(packet.threshold.0);
                match switch_mode {
                    SwitchMode::Immediate => io.set_compression(threshold),
                    SwitchMode::Acknowledged => stored_threshold = threshold,
                }
                None
            }
            ConnectionState::Login if id == ClientboundLoginFinished::ID => {
                acknowledge_login(&mut io, switch_mode, stored_threshold, &body).await?;
                state = ConnectionState::Configuration;
                None
            }
            ConnectionState::Login => anyhow::bail!("unexpected login packet {id:#04x}"),
            ConnectionState::Configuration if id == ConfigurationDisconnect::ID => {
                let packet: ConfigurationDisconnect = decode(&body)?;
                Some(packet.reason)
            }
            ConnectionState::Configuration => {
                if configuration.read(id, &body, true, &mut io)?.is_some() {
                    io.write_packet_fallible(&ServerboundFinishConfiguration)?;
                    state = ConnectionState::Play;
                }
                io.flush().await?;
                None
            }
            ConnectionState::Play if id == ClientboundDisconnect::ID => {
                let packet: ClientboundDisconnect = decode(&body)?;
                Some(packet.reason)
            }
            ConnectionState::Play if id == ClientboundLogin::ID => {
                let packet: ClientboundLogin = decode(&body)?;
                return Ok(Login::Joined(Joined {
                    io: Box::new(io),
                    player_id: packet.player_id,
                }));
            }
            ConnectionState::Play => None,
        };
        if reason.is_some() {
            return Ok(Login::Refused { state, reason });
        }
    }
}

async fn acknowledge_login(
    io: &mut PacketIo<TcpStream>,
    switch_mode: SwitchMode,
    stored_threshold: CompressionThreshold,
    body: &[u8],
) -> anyhow::Result<()> {
    let options = match switch_mode {
        SwitchMode::Immediate => OptionFlags::new(),
        SwitchMode::Acknowledged => {
            let packet: ClientboundLoginFinishedWithOptions = decode(body)?;
            packet.options
        }
    };
    anyhow::ensure!(!options.cipher(), "the server asked for encryption");
    let compression = options.compression().then_some(stored_threshold);
    if let Some(threshold) = compression {
        io.set_inbound_compression(threshold);
    }
    io.send_packet(&ServerboundLoginAcknowledged).await?;
    if let Some(threshold) = compression {
        io.set_outbound_compression(threshold);
    }
    io.send_packet(&ConfigClientInformation(client_information(MainArm::Right)))
        .await?;
    Ok(())
}

pub(crate) fn needs_world_acks(observe: &Observe) -> bool {
    observe.inventory || observe.swing || observe.animate || observe.packet_order
}

fn client_information(main_arm: MainArm) -> ClientInformation<'static> {
    ClientInformation {
        locale: "en_us",
        view_distance: 2,
        chat_mode: ChatMode::Enabled,
        chat_colors: true,
        displayed_skin_parts: DisplayedSkinParts::from_bits(0x7f),
        main_arm,
        enable_text_filtering: false,
        allow_server_listings: true,
        particle_status: ParticleStatus::All,
    }
}

pub(crate) struct SessionConfig {
    pub(crate) observe: Observe,
    pub(crate) tracks_pose: bool,
    pub(crate) known_players: Vec<String>,
    pub(crate) load_signal: LoadSignal,
}

pub(crate) async fn session(
    joined: Joined,
    client: String,
    config: SessionConfig,
    mut orders: UnboundedReceiver<QueuedOrder>,
    events: UnboundedSender<Event>,
) {
    let mut io = joined.io;
    let SessionConfig {
        observe,
        tracks_pose,
        known_players,
        load_signal,
    } = config;
    if load_signal == LoadSignal::Send
        && needs_world_acks(&observe)
        && let Err(error) = automatic_reply(&mut io, &ServerboundPlayerLoaded).await
    {
        let signal = failure(&client, &error);
        let _ = events.send(Event::now(client, signal));
        return;
    }
    let mut state = SessionState {
        client: &client,
        observe: &observe,
        tracks_pose,
        view: MenuView::default(),
        pose: Pose::default(),
        names: EntityNames::with_players(&known_players),
        awaiting_respawn_pos: false,
    };
    state.names.record_login(&client, joined.player_id);
    let mut pending_orders: VecDeque<QueuedOrder> = VecDeque::new();
    let signal = loop {
        tokio::select! {
            frame = io.recv_frame() => {
                let (id, body) = match frame {
                    Ok(frame) => frame,
                    Err(error) if is_close(&error) => break Signal::Closed,
                    Err(error) => break failure(&client, &error),
                };
                match read_game_packet(&mut io, &mut state, id, &body).await {
                    Ok(Some(
                        signal @ (Signal::Chat(_)
                        | Signal::Menu(_)
                        | Signal::Sound(_)
                        | Signal::Commands(_)
                        | Signal::Suggestions { .. }
                        | Signal::Swing(_)
                        | Signal::BlockAck(_)
                        | Signal::Added(_)
                        | Signal::Removed(_)
                        | Signal::PositionAccepted(_)
                        | Signal::Animate(_)
                        | Signal::Packet(_)),
                    )) => {
                        if events.send(Event::now(client.clone(), signal)).is_err() {
                            return;
                        }
                    }
                    Ok(Some(signal)) => break signal,
                    Ok(None) => {}
                    Err(error) => break failure(&client, &error),
                }
                if !state.awaiting_respawn_pos
                    && let ControlFlow::Break(signal) =
                        drain_pending(&mut io, &mut state, &client, &mut pending_orders).await
                {
                    break signal;
                }
            }
            queued = orders.recv() => {
                let Some(queued) = queued else {
                    let _ = time::timeout(CLOSE_LIMIT, io.close()).await;
                    break Signal::Closed;
                };
                if matches!(queued.order, Order::Leave) {
                    let _ = time::timeout(CLOSE_LIMIT, io.close()).await;
                    if let Some(done) = queued.written {
                        let _ = done.send(Ok(()));
                    }
                    break Signal::Closed;
                }
                if state.awaiting_respawn_pos
                    && (!pending_orders.is_empty()
                        || matches!(
                            queued.order,
                            Order::Attack(_)
                                | Order::Interact { .. }
                                | Order::SyncPosition
                                | Order::UseItem { .. }
                        ))
                {
                    pending_orders.push_back(queued);
                    continue;
                }
                if let ControlFlow::Break(signal) =
                    dispatch_queued(&mut io, &mut state, &client, queued).await
                {
                    break signal;
                }
            }
        }
    };
    let _ = events.send(Event::now(client, signal));
}

async fn drain_pending(
    io: &mut PacketIo<TcpStream>,
    state: &mut SessionState<'_>,
    client: &str,
    pending: &mut VecDeque<QueuedOrder>,
) -> ControlFlow<Signal> {
    while let Some(queued) = pending.pop_front() {
        dispatch_queued(io, state, client, queued).await?;
    }
    ControlFlow::Continue(())
}

async fn dispatch_queued(
    io: &mut PacketIo<TcpStream>,
    state: &mut SessionState<'_>,
    client: &str,
    queued: QueuedOrder,
) -> ControlFlow<Signal> {
    if queued
        .written
        .as_ref()
        .is_some_and(oneshot::Sender::is_closed)
    {
        return ControlFlow::Continue(());
    }
    let Some(budget) = queued
        .deadline
        .checked_duration_since(time::Instant::now())
        .filter(|remaining| !remaining.is_zero())
    else {
        if let Some(done) = queued.written {
            let _ = done.send(Err(format!("{client} order deadline expired before write")));
        }
        return ControlFlow::Continue(());
    };
    let result = match time::timeout(budget, send_order(io, state, queued.order)).await {
        Ok(res) => res,
        Err(_) => Err(anyhow::anyhow!(
            "{client} packet write timed out after {budget:?}"
        )),
    };
    match result {
        Ok(()) => {
            if let Some(done) = queued.written {
                let _ = done.send(Ok(()));
            }
            ControlFlow::Continue(())
        }
        Err(error) => {
            let message = format!("{error:#}");
            if let Some(done) = queued.written {
                let _ = done.send(Err(message));
            }
            ControlFlow::Break(failure(client, &error))
        }
    }
}

fn failure(client: &str, error: &anyhow::Error) -> Signal {
    eprintln!("{client}: {error:#}");
    Signal::Failed(format!("{error:#}"))
}

struct SessionState<'a> {
    client: &'a str,
    observe: &'a Observe,
    tracks_pose: bool,
    view: MenuView,
    pose: Pose,
    names: EntityNames,
    awaiting_respawn_pos: bool,
}

#[derive(Debug)]
struct PunchWithExtraByte;

impl Packet for PunchWithExtraByte {
    const ID: i32 = ServerboundPunch::ID;
    const NAME: &'static str = ServerboundPunch::NAME;
    const SIDE: mcrs_minecraft_protocol::PacketSide = ServerboundPunch::SIDE;
    const STATE: mcrs_minecraft_protocol::ConnectionState = ServerboundPunch::STATE;
}

impl Encode for PunchWithExtraByte {
    fn encode(&self, mut out: impl Write) -> mcrs_minecraft_protocol::Result<()> {
        out.write_all(&[0x01])?;
        Ok(())
    }
}

async fn send_order(
    io: &mut PacketIo<TcpStream>,
    state: &mut SessionState<'_>,
    order: Order,
) -> anyhow::Result<()> {
    match order {
        Order::Command(line) => {
            io.send_packet(&ServerboundChatCommand {
                command: Bounded(line.as_str()),
            })
            .await
        }
        Order::Suggest { id, line } => {
            io.send_packet(&ServerboundCommandSuggestion {
                id: VarInt(id),
                text: Bounded(line.as_str()),
            })
            .await
        }
        Order::ClientMainArm(arm) => {
            io.send_packet(&GameClientInformation(client_information(arm)))
                .await
        }
        Order::PunchExtraByte => io.send_packet(&PunchWithExtraByte).await,
        Order::Leave => Ok(()),
        Order::SendLoaded => io.send_packet(&ServerboundPlayerLoaded).await,
        Order::SyncPosition => send_sync_position(io, state).await,
        Order::Click(click) => send_click(io, &state.view, click).await,
        Order::Close(container) => {
            let container_id = VarInt(container.unwrap_or(state.view.container()));
            state.view.close();
            io.send_packet(&ServerboundContainerClose { container_id })
                .await
        }
        Order::SelectSlot(slot) => {
            let slot = slot.cast_unsigned();
            io.send_packet(&ServerboundSetCarriedItem { slot }).await
        }
        Order::PlayerAction {
            action,
            position: [x, y, z],
            face,
        } => {
            io.send_packet(&ServerboundPlayerAction {
                action,
                pos: BlockPos::new(x, y, z),
                direction: face,
                sequence: VarInt(0),
            })
            .await
        }
        Order::CreativeSlot { slot, stack } => {
            io.send_packet(&ServerboundSetCreativeModeSlot {
                slot,
                item: PredictedStack::delimited(stack),
            })
            .await
        }
        Order::UseBlock(position) => {
            send_use_item_on(io, Hand::Main, position, Direction::Up, 0).await
        }
        Order::Punch => io.send_packet(&ServerboundPunch).await,
        Order::PunchBurst(count) => {
            for _ in 0..count {
                io.send_packet(&ServerboundPunch).await?;
            }
            Ok(())
        }
        Order::UseItem { hand, sequence } => {
            io.send_packet(&ServerboundUseItem {
                hand,
                sequence: VarInt(sequence),
                look: state.pose.look(),
            })
            .await
        }
        Order::UseItemOn {
            hand,
            position,
            face,
            sequence,
        } => send_use_item_on(io, hand, position, face, sequence).await,
        Order::Attack(target) => {
            let entity_id = resolve_target_id(state, &target)?;
            io.send_packet(&ServerboundAttack { entity_id }).await
        }
        Order::Interact {
            target,
            hand,
            secondary,
        } => {
            let entity_id = resolve_target_id(state, &target)?;
            io.send_packet(&ServerboundInteract {
                entity_id,
                hand,
                location: LpVec3::default(),
                using_secondary_action: secondary,
            })
            .await
        }
    }
}

fn resolve_target_id(state: &SessionState<'_>, target: &str) -> anyhow::Result<VarInt> {
    state
        .names
        .target_id(target, state.pose.accepted_position())
}

async fn send_sync_position(
    io: &mut PacketIo<TcpStream>,
    state: &SessionState<'_>,
) -> anyhow::Result<()> {
    match state.pose.move_pos_rot() {
        Some(packet) => io.send_packet(&packet).await,
        None => anyhow::bail!(
            "{} cannot sync_position before accepting a player_position packet",
            state.client
        ),
    }
}

async fn send_click(
    io: &mut PacketIo<TcpStream>,
    view: &MenuView,
    click: Click,
) -> anyhow::Result<()> {
    let changed_slots = click
        .changed_slots
        .iter()
        .map(|changed| (changed.slot, changed.stack.map(PredictedStack::hashed)))
        .collect();
    let container = click.container.unwrap_or(view.container());
    let state_id = view
        .state_id(container)
        .checked_add(click.state_id_offset)
        .context("the state id offset overflows")?;
    let clicked = Clicked::from(click.slot);
    anyhow::ensure!(
        i16::try_from(clicked).ok() == Some(click.slot),
        "the slot {} has no click on the wire",
        click.slot
    );
    io.send_packet(&ServerboundContainerClick {
        container_id: VarInt(container),
        state_seqno: VarInt(state_id),
        click: mcrs_minecraft_container_click::Click {
            kind: click.mode.into(),
            button: click.button,
            clicked,
        },
        changed_slots: Bounded(changed_slots),
        carried_item: click.carried.map(PredictedStack::hashed),
    })
    .await
}

struct FaceHitPoint {
    x: f32,
    y: f32,
    z: f32,
}

fn face_hit_point(face: Direction) -> FaceHitPoint {
    match face {
        Direction::Down => FaceHitPoint {
            x: 0.5,
            y: 0.0,
            z: 0.5,
        },
        Direction::Up => FaceHitPoint {
            x: 0.5,
            y: 1.0,
            z: 0.5,
        },
        Direction::North => FaceHitPoint {
            x: 0.5,
            y: 0.5,
            z: 0.0,
        },
        Direction::South => FaceHitPoint {
            x: 0.5,
            y: 0.5,
            z: 1.0,
        },
        Direction::West => FaceHitPoint {
            x: 0.0,
            y: 0.5,
            z: 0.5,
        },
        Direction::East => FaceHitPoint {
            x: 1.0,
            y: 0.5,
            z: 0.5,
        },
    }
}

async fn send_use_item_on(
    io: &mut PacketIo<TcpStream>,
    hand: Hand,
    [x, y, z]: [i32; 3],
    face: Direction,
    sequence: i32,
) -> anyhow::Result<()> {
    let hit = face_hit_point(face);
    io.send_packet(&ServerboundUseItemOn {
        hand,
        block_pos: BlockPos::new(x, y, z),
        face,
        cursor_x: hit.x,
        cursor_y: hit.y,
        cursor_z: hit.z,
        inside_block: false,
        world_border_hit: false,
        sequence: VarInt(sequence),
    })
    .await
}

async fn read_game_packet(
    io: &mut PacketIo<TcpStream>,
    state: &mut SessionState<'_>,
    id: i32,
    body: &[u8],
) -> anyhow::Result<Option<Signal>> {
    if id == ClientboundKeepAlive::ID {
        let packet: ClientboundKeepAlive = decode(body)?;
        let payload = packet.0.payload;
        automatic_reply(io, &ServerboundKeepAlive(KeepAlive { payload })).await?;
    } else if id == ClientboundSystemChatPacket::ID {
        let packet: ClientboundSystemChatPacket = decode(body)?;
        if !packet.overlay {
            return Ok(Some(Signal::Chat(packet.content.to_text()?)));
        }
    } else if id == ClientboundDisconnect::ID {
        let packet: ClientboundDisconnect = decode(body)?;
        return Ok(Some(Signal::Disconnected(packet.reason)));
    } else if id == ClientboundCommandSuggestions::ID {
        let packet: ClientboundCommandSuggestions<'_> = decode(body)?;
        return Ok(Some(Signal::Suggestions {
            id: packet.id.0,
            start: packet.start.0,
            length: packet.length.0,
            suggestions: packet
                .suggestions
                .into_iter()
                .map(|entry| entry.text.into_owned())
                .collect(),
        }));
    } else if state.observe.sound && SoundPlayed::is_packet(id) {
        return Ok(Some(Signal::Sound(SoundPlayed::read(state.client, body)?)));
    } else if !state.observe.commands.is_empty() && id == ClientboundCommands::ID {
        let tree = CommandTree::read(state.client, body, &state.observe.commands)?;
        return Ok(Some(Signal::Commands(tree)));
    } else if state.observe.swing && SwingObserved::is_packet(id) {
        return Ok(Some(Signal::Swing(SwingObserved::read(
            state.client,
            &state.names,
            body,
        )?)));
    } else if state.observe.animate && AnimateObserved::is_packet(id) {
        return Ok(Some(Signal::Animate(AnimateObserved::read(
            state.client,
            &state.names,
            body,
        )?)));
    } else if state.observe.packet_order
        && let Some(packet) = packet_order_kind(id)
    {
        return Ok(Some(Signal::Packet(packet)));
    } else if id == ClientboundAddEntity::ID {
        let packet: ClientboundAddEntity = decode(body)?;
        state.names.record_add_entity(&packet);
        return Ok(Some(Signal::Added(
            state.names.resolve_subject(packet.id.0),
        )));
    } else if id == ClientboundRemoveEntities::ID {
        let packet: ClientboundRemoveEntities = decode(body)?;
        return Ok(Some(Signal::Removed(state.names.remove_entities(&packet))));
    } else if id == ClientboundBlockChangedAck::ID {
        let packet: ClientboundBlockChangedAck = decode(body)?;
        return Ok(Some(Signal::BlockAck(packet.sequence.0)));
    } else if id == ClientboundTeleportEntity::ID {
        let packet: ClientboundTeleportEntity = decode(body)?;
        state.names.record_teleport(&packet);
    } else if id == ClientboundEntityPositionSync::ID {
        let packet: ClientboundEntityPositionSync = decode(body)?;
        state.names.record_position_sync(&packet);
    } else if id == ClientboundMoveEntityPos::ID {
        let packet: ClientboundMoveEntityPos = decode(body)?;
        state
            .names
            .record_move_delta(packet.entity_id.0, &packet.delta);
    } else if id == ClientboundMoveEntityPosRot::ID {
        let packet: ClientboundMoveEntityPosRot = decode(body)?;
        state
            .names
            .record_move_delta(packet.entity_id.0, &packet.delta);
    } else if id == ClientboundRespawn::ID {
        let _: ClientboundRespawn = decode(body)?;
        state.names.clear_world_entities();
        state.pose.clear_accepted();
        state.awaiting_respawn_pos = state.tracks_pose;
    } else if state.tracks_pose && id == ClientboundPlayerPosition::ID {
        return accept_position(io, state, body).await.map(Some);
    } else if needs_world_acks(state.observe) && id == ClientboundChunkBatchFinished::ID {
        automatic_reply(
            io,
            &ServerboundChunkBatchReceived {
                desired_chunks_per_tick: CHUNKS_PER_TICK,
            },
        )
        .await?;
    } else if !state.observe.inventory {
        return Ok(None);
    } else if let Some(packet) = MenuPacket::read(id, body)? {
        state.view.observe(&packet);
        return Ok(Some(Signal::Menu(packet)));
    }
    Ok(None)
}

async fn accept_position(
    io: &mut PacketIo<TcpStream>,
    state: &mut SessionState<'_>,
    body: &[u8],
) -> anyhow::Result<Signal> {
    let packet: ClientboundPlayerPosition = decode(body)?;
    let acceptance = state.pose.accept(&packet);
    automatic_reply(io, &acceptance).await?;
    state.awaiting_respawn_pos = false;
    Ok(Signal::PositionAccepted(acceptance.position))
}

async fn automatic_reply<P>(io: &mut PacketIo<TcpStream>, packet: &P) -> anyhow::Result<()>
where
    P: Packet + Encode,
{
    time::timeout(WRITE_LIMIT, io.send_packet(packet))
        .await
        .context("automatic packet write timed out")?
}

fn packet_order_kind(id: i32) -> Option<PacketKind> {
    if SwingObserved::is_packet(id) {
        Some(PacketKind::SwingAnimation)
    } else if SoundPlayed::is_packet(id) {
        Some(PacketKind::Sound)
    } else if id == ClientboundBlockUpdate::ID {
        Some(PacketKind::BlockUpdate)
    } else if id == ClientboundSectionBlocksUpdate::ID {
        Some(PacketKind::SectionBlocksUpdate)
    } else if id == ClientboundSetEquipment::ID {
        Some(PacketKind::SetEquipment)
    } else if id == ClientboundSetEntityData::ID {
        Some(PacketKind::SetEntityData)
    } else {
        None
    }
}

fn decode<'a, P: Decode<'a>>(mut body: &'a [u8]) -> anyhow::Result<P> {
    Ok(P::decode(&mut body)?)
}

fn is_close(error: &anyhow::Error) -> bool {
    error.downcast_ref::<io::Error>().is_some_and(|error| {
        matches!(
            error.kind(),
            io::ErrorKind::UnexpectedEof
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
        )
    })
}
