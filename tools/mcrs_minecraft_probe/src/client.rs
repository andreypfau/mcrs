use crate::menu_view::MenuView;
use crate::pose::Pose;
use crate::{
    Click, CommandTree, ConnectionState, MenuPacket, Observe, PredictedStack, SoundPlayed,
};
use anyhow::Context;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_network::SwitchMode;
use mcrs_minecraft_network::join::Configuration;
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::common::serverbound::{ClientInformation, KeepAlive};
use mcrs_minecraft_protocol::packets::configuration::clientbound::ClientboundDisconnect as ConfigurationDisconnect;
use mcrs_minecraft_protocol::packets::configuration::serverbound::{
    ServerboundClientInformation, ServerboundFinishConfiguration,
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
use mcrs_minecraft_protocol::{Bounded, CompressionThreshold, Decode, Packet, VarInt, WritePacket};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundChunkBatchFinished, ClientboundCommands, ClientboundLogin,
    ClientboundPlayerPosition, ClientboundSystemChatPacket,
};
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundChatCommand, ServerboundChunkBatchReceived, ServerboundContainerClick,
    ServerboundContainerClose, ServerboundPlayerAction, ServerboundPlayerLoaded,
    ServerboundSetCarriedItem, ServerboundSetCreativeModeSlot, ServerboundUseItemOn,
};
use mcrs_minecraft_protocol_game::{Direction, Hand};
use mcrs_minecraft_text::Text;
use mcrs_voxel::BlockPos;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time;

const CLOSE_LIMIT: Duration = Duration::from_secs(5);
const CHUNKS_PER_TICK: f32 = 64.0;

pub(crate) enum Login {
    Joined(Box<PacketIo<TcpStream>>),
    Refused {
        state: ConnectionState,
        reason: Option<Text>,
    },
}

pub(crate) enum Order {
    Command(String),
    Leave,
    Click(Click),
    Close(Option<i32>),
    SelectSlot(i16),
    PlayerAction(PlayerAction),
    CreativeSlot {
        slot: i16,
        stack: Option<PredictedStack>,
    },
    UseBlock([i32; 3]),
}

pub(crate) struct Event {
    pub(crate) client: String,
    pub(crate) signal: Signal,
}

pub(crate) enum Signal {
    Chat(Text),
    Menu(MenuPacket),
    Sound(SoundPlayed),
    Commands(CommandTree),
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
                return Ok(Login::Joined(Box::new(io)));
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
    io.send_packet(&ServerboundClientInformation(ClientInformation {
        locale: "en_us",
        view_distance: 2,
        chat_mode: ChatMode::Enabled,
        chat_colors: true,
        displayed_skin_parts: DisplayedSkinParts::from_bits(0x7f),
        main_arm: MainArm::Right,
        enable_text_filtering: false,
        allow_server_listings: true,
        particle_status: ParticleStatus::All,
    }))
    .await?;
    Ok(())
}

pub(crate) async fn session(
    mut io: PacketIo<TcpStream>,
    client: String,
    observe: Observe,
    mut orders: UnboundedReceiver<Order>,
    events: UnboundedSender<Event>,
) {
    if observe.inventory
        && let Err(error) = io.send_packet(&ServerboundPlayerLoaded).await
    {
        let signal = failure(&client, &error);
        let _ = events.send(Event { client, signal });
        return;
    }
    let mut view = MenuView::default();
    let mut pose = Pose::default();
    let signal = loop {
        tokio::select! {
            frame = io.recv_frame() => {
                let Ok((id, body)) = frame else {
                    break Signal::Closed;
                };
                match read_game_packet(&mut io, &client, &observe, &mut view, &mut pose, id, &body).await {
                    Ok(Some(
                        signal @ (Signal::Chat(_)
                        | Signal::Menu(_)
                        | Signal::Sound(_)
                        | Signal::Commands(_)),
                    )) => {
                        if events.send(Event { client: client.clone(), signal }).is_err() {
                            return;
                        }
                    }
                    Ok(Some(signal)) => break signal,
                    Ok(None) => {}
                    Err(error) => break failure(&client, &error),
                }
            }
            order = orders.recv() => {
                let order = match order {
                    Some(Order::Leave) | None => {
                        let _ = time::timeout(CLOSE_LIMIT, io.close()).await;
                        break Signal::Closed;
                    }
                    Some(order) => order,
                };
                if let Err(error) = send_order(&mut io, &mut view, order).await {
                    break failure(&client, &error);
                }
            }
        }
    };
    let _ = events.send(Event { client, signal });
}

fn failure(client: &str, error: &anyhow::Error) -> Signal {
    eprintln!("{client}: {error:#}");
    Signal::Failed(format!("{error:#}"))
}

async fn send_order(
    io: &mut PacketIo<TcpStream>,
    view: &mut MenuView,
    order: Order,
) -> anyhow::Result<()> {
    match order {
        Order::Command(line) => {
            io.send_packet(&ServerboundChatCommand {
                command: Bounded(line.as_str()),
            })
            .await
        }
        Order::Leave => Ok(()),
        Order::Click(click) => {
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
            io.send_packet(&ServerboundContainerClick {
                container_id: VarInt(container),
                state_seqno: VarInt(state_id),
                slot_index: click.slot,
                button: click.button,
                container_input: click.mode.into(),
                changed_slots: Bounded(changed_slots),
                carried_item: click.carried.map(PredictedStack::hashed),
            })
            .await
        }
        Order::Close(container) => {
            let container_id = VarInt(container.unwrap_or(view.container()));
            view.close();
            io.send_packet(&ServerboundContainerClose { container_id })
                .await
        }
        Order::SelectSlot(slot) => {
            let slot = slot.cast_unsigned();
            io.send_packet(&ServerboundSetCarriedItem { slot }).await
        }
        Order::PlayerAction(action) => {
            io.send_packet(&ServerboundPlayerAction {
                action,
                pos: BlockPos::new(0, 0, 0),
                direction: Direction::Down,
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
        Order::UseBlock([x, y, z]) => {
            io.send_packet(&ServerboundUseItemOn {
                hand: Hand::Main,
                block_pos: BlockPos::new(x, y, z),
                face: Direction::Up,
                cursor_x: 0.5,
                cursor_y: 1.0,
                cursor_z: 0.5,
                inside_block: false,
                world_border_hit: false,
                sequence: VarInt(0),
            })
            .await
        }
    }
}

async fn read_game_packet(
    io: &mut PacketIo<TcpStream>,
    client: &str,
    observe: &Observe,
    view: &mut MenuView,
    pose: &mut Pose,
    id: i32,
    body: &[u8],
) -> anyhow::Result<Option<Signal>> {
    if id == ClientboundKeepAlive::ID {
        let packet: ClientboundKeepAlive = decode(body)?;
        let payload = packet.0.payload;
        io.send_packet(&ServerboundKeepAlive(KeepAlive { payload }))
            .await?;
    } else if id == ClientboundSystemChatPacket::ID {
        let packet: ClientboundSystemChatPacket = decode(body)?;
        if !packet.overlay {
            return Ok(Some(Signal::Chat(packet.content.to_text()?)));
        }
    } else if id == ClientboundDisconnect::ID {
        let packet: ClientboundDisconnect = decode(body)?;
        return Ok(Some(Signal::Disconnected(packet.reason)));
    } else if observe.sound && SoundPlayed::is_packet(id) {
        return Ok(Some(Signal::Sound(SoundPlayed::read(client, body)?)));
    } else if !observe.commands.is_empty() && id == ClientboundCommands::ID {
        let tree = CommandTree::read(client, body, &observe.commands)?;
        return Ok(Some(Signal::Commands(tree)));
    } else if !observe.inventory {
        return Ok(None);
    } else if id == ClientboundPlayerPosition::ID {
        let packet: ClientboundPlayerPosition = decode(body)?;
        io.send_packet(&pose.accept(&packet)).await?;
    } else if id == ClientboundChunkBatchFinished::ID {
        io.send_packet(&ServerboundChunkBatchReceived {
            desired_chunks_per_tick: CHUNKS_PER_TICK,
        })
        .await?;
    } else if let Some(packet) = MenuPacket::read(id, body)? {
        view.observe(&packet);
        return Ok(Some(Signal::Menu(packet)));
    }
    Ok(None)
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
                | io::ErrorKind::BrokenPipe
        )
    })
}
