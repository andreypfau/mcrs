use crate::ConnectionState;
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
use mcrs_minecraft_protocol_game::clientbound::{ClientboundLogin, ClientboundSystemChatPacket};
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_text::Text;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time;

const CLOSE_LIMIT: Duration = Duration::from_secs(5);

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
}

pub(crate) struct Event {
    pub(crate) client: String,
    pub(crate) signal: Signal,
}

pub(crate) enum Signal {
    Chat(Text),
    Disconnected(Text),
    Closed,
}

/// Logs in offline and configures, until the first packet of the game state or a refusal.
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

/// Answers the end of the login and sends the client information of the configuration.
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

/// Keeps one client in the game state: answers keep-alives, reports what the driver records and
/// carries out its orders, until the connection ends.
pub(crate) async fn session(
    mut io: PacketIo<TcpStream>,
    client: String,
    mut orders: UnboundedReceiver<Order>,
    events: UnboundedSender<Event>,
) {
    let signal = loop {
        tokio::select! {
            frame = io.recv_frame() => {
                let Ok((id, body)) = frame else {
                    break Signal::Closed;
                };
                match read_game_packet(&mut io, id, &body).await {
                    Ok(Some(Signal::Chat(text))) => {
                        let event = Event { client: client.clone(), signal: Signal::Chat(text) };
                        if events.send(event).is_err() {
                            return;
                        }
                    }
                    Ok(Some(signal)) => break signal,
                    Ok(None) => {}
                    Err(error) => {
                        eprintln!("{client}: {error:#}");
                        break Signal::Closed;
                    }
                }
            }
            order = orders.recv() => {
                if let Some(Order::Command(line)) = order {
                    let packet = ServerboundChatCommand { command: Bounded(line.as_str()) };
                    if io.send_packet(&packet).await.is_err() {
                        break Signal::Closed;
                    }
                } else {
                    let _ = time::timeout(CLOSE_LIMIT, io.close()).await;
                    break Signal::Closed;
                }
            }
        }
    };
    let _ = events.send(Event { client, signal });
}

async fn read_game_packet(
    io: &mut PacketIo<TcpStream>,
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
