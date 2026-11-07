use std::collections::VecDeque;
use std::io;
use std::io::Read;
use std::io::Write;
use std::mem;
use std::net::TcpStream;
use std::time::Duration;

use bytes::BufMut;
use bytes::Bytes;
use bytes::BytesMut;
use flate2::Decompress;
use mcrs_minecraft_client_ticks::Event;
use mcrs_minecraft_protocol::CompressionThreshold;
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::Encode;
use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::frame::Deflate;
use mcrs_minecraft_protocol::frame::decompress;
use mcrs_minecraft_protocol::frame::encode_frame;
use mcrs_minecraft_protocol::frame::split_frame;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::login::clientbound::LoginCompression;
use mcrs_minecraft_protocol::packets::table::configuration_clientbound;
use mcrs_minecraft_protocol::packets::table::configuration_serverbound;
use mcrs_minecraft_protocol::packets::table::game_clientbound;
use mcrs_minecraft_protocol::packets::table::game_serverbound;
use mcrs_minecraft_protocol::packets::table::login_clientbound;
use mcrs_minecraft_protocol::packets::table::login_serverbound;
use mcrs_minecraft_protocol_game::clientbound::ClientboundPing;

use crate::names::Names;
use crate::play::Play;
use crate::play::decode_clientbound;
use crate::play::decode_serverbound;
use crate::play::name_of;

const POLL: Duration = Duration::from_millis(1);

/// What the proxy saw, in the order the client applied or sent it.
pub(crate) enum Observed {
    /// A clientbound event; the client applied it before its next tick.
    Applied(Event),
    /// A serverbound event.
    Sent(Event),
    /// The client has loaded the world.
    Loaded,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum State {
    Handshake,
    Status,
    Login,
    Configuration,
    Play,
}

/// One peer: the bytes from it, cut into frames, and the frames for it.
struct Inbound {
    stream: TcpStream,
    buffer: BytesMut,
    outgoing: Vec<u8>,
    inflater: Option<Decompress>,
    state: State,
}

/// A `ping` that the proxy sent after the events it brackets.
struct Probe {
    id: i32,
    events: Vec<Event>,
}

/// One client connection, forwarded to the server frame by frame.
///
/// After each clientbound packet that the recording keeps, the proxy sends its own `ping`. The
/// client answers a `ping` between two client ticks, after it applied the packets before it, so
/// the answer gives the tick that applied them. The proxy drops these answers.
pub(crate) struct Connection {
    client: Inbound,
    server: Inbound,
    threshold: CompressionThreshold,
    deflate: Option<Deflate>,
    in_bundle: bool,
    unprobed: Vec<Event>,
    probes: VecDeque<Probe>,
    next_probe: i32,
    /// The network id of the player, from `login`.
    player: Option<i32>,
}

impl Connection {
    pub(crate) fn new(client: TcpStream, server: TcpStream) -> anyhow::Result<Self> {
        for stream in [&client, &server] {
            stream.set_read_timeout(Some(POLL))?;
            stream.set_nodelay(true)?;
        }
        let inbound = |stream| Inbound {
            stream,
            buffer: BytesMut::new(),
            outgoing: Vec::new(),
            inflater: None,
            state: State::Handshake,
        };
        Ok(Self {
            client: inbound(client),
            server: inbound(server),
            threshold: CompressionThreshold::default(),
            deflate: None,
            in_bundle: false,
            unprobed: Vec::new(),
            probes: VecDeque::new(),
            next_probe: i32::MIN,
            player: None,
        })
    }

    /// Forwards what both peers sent since the last call. Returns false once a peer closed.
    pub(crate) fn poll(&mut self, names: &Names, out: &mut Vec<Observed>) -> anyhow::Result<bool> {
        if !receive(&mut self.client)? {
            return Ok(false);
        }
        while let Some(frame) = next_frame(&mut self.client)? {
            self.client_frame(frame, out)?;
        }
        flush(&mut self.server)?;
        if !receive(&mut self.server)? {
            return Ok(false);
        }
        while let Some(frame) = next_frame(&mut self.server)? {
            self.server_frame(names, frame)?;
        }
        flush(&mut self.client)?;
        Ok(true)
    }

    fn client_frame(&mut self, frame: Bytes, out: &mut Vec<Observed>) -> anyhow::Result<()> {
        let packet = decompress(frame.clone(), self.threshold, &mut self.client.inflater)?;
        let mut body = &packet[..];
        let id = VarInt::decode(&mut body)?.0;
        match self.client.state {
            State::Handshake => {
                let state = match ServerboundHandshake::decode(&mut body)?.intent {
                    Intent::Status => State::Status,
                    Intent::Login | Intent::Transfer => State::Login,
                };
                self.client.state = state;
                self.server.state = state;
            }
            State::Login if name_of(login_serverbound::NAMES, id) == "login_acknowledged" => {
                self.client.state = State::Configuration;
            }
            State::Configuration
                if name_of(configuration_serverbound::NAMES, id) == "finish_configuration" =>
            {
                self.client.state = State::Play;
            }
            State::Play if name_of(game_serverbound::NAMES, id) == "configuration_acknowledged" => {
                self.client.state = State::Configuration;
            }
            State::Play => match decode_serverbound(id, body)? {
                Play::Ping(probe) if self.probes.front().is_some_and(|front| front.id == probe) => {
                    if let Some(answered) = self.probes.pop_front() {
                        out.extend(answered.events.into_iter().map(Observed::Applied));
                    }
                    return Ok(());
                }
                Play::Loaded => out.push(Observed::Loaded),
                Play::Events(events) => out.extend(events.into_iter().map(Observed::Sent)),
                Play::Login(_) | Play::Ping(_) | Play::Bundle => {}
            },
            State::Status | State::Login | State::Configuration => {}
        }
        send_frame(&mut self.server.outgoing, &frame)
    }

    fn server_frame(&mut self, names: &Names, frame: Bytes) -> anyhow::Result<()> {
        let packet = decompress(frame.clone(), self.threshold, &mut self.server.inflater)?;
        let mut body = &packet[..];
        let id = VarInt::decode(&mut body)?.0;
        let mut compression = None;
        match self.server.state {
            State::Login => match name_of(login_clientbound::NAMES, id) {
                "setup_compression" => {
                    compression = Some(LoginCompression::decode(&mut body)?.threshold.0);
                }
                "login_finished" => self.server.state = State::Configuration,
                _ => {}
            },
            State::Configuration
                if name_of(configuration_clientbound::NAMES, id) == "finish_configuration" =>
            {
                self.server.state = State::Play;
            }
            State::Play if name_of(game_clientbound::NAMES, id) == "start_configuration" => {
                self.server.state = State::Configuration;
            }
            State::Play => match decode_clientbound(names, self.player, id, body)? {
                Play::Login(player) => self.player = Some(player),
                Play::Bundle => self.in_bundle = !self.in_bundle,
                Play::Events(events) => self.unprobed.extend(events),
                Play::Loaded | Play::Ping(_) => {}
            },
            State::Handshake | State::Status | State::Configuration => {}
        }
        send_frame(&mut self.client.outgoing, &frame)?;
        if let Some(threshold) = compression {
            self.threshold = CompressionThreshold(threshold);
        }
        if !self.unprobed.is_empty() && !self.in_bundle {
            self.probe()?;
        }
        Ok(())
    }

    fn probe(&mut self) -> anyhow::Result<()> {
        let id = self.next_probe;
        self.next_probe = self.next_probe.wrapping_add(1);
        let mut packet = BytesMut::new();
        ClientboundPing { id }.encode_with_id((&mut packet).writer())?;
        encode_frame(&mut packet, 0, self.threshold, &mut self.deflate)?;
        self.client.outgoing.extend_from_slice(&packet);
        self.probes.push_back(Probe {
            id,
            events: mem::take(&mut self.unprobed),
        });
        Ok(())
    }
}

/// Reads what the peer sent, until a read does not fill the chunk. Returns false once the peer
/// closed.
fn receive(inbound: &mut Inbound) -> anyhow::Result<bool> {
    let mut chunk = [0; 64 * 1024];
    loop {
        match inbound.stream.read(&mut chunk) {
            Ok(0) => return Ok(false),
            Ok(read) => {
                inbound
                    .buffer
                    .extend_from_slice(chunk.get(..read).unwrap_or_default());
                if read < chunk.len() {
                    return Ok(true);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Ok(true);
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn flush(inbound: &mut Inbound) -> anyhow::Result<()> {
    inbound.stream.write_all(&inbound.outgoing)?;
    inbound.outgoing.clear();
    Ok(())
}

fn next_frame(inbound: &mut Inbound) -> anyhow::Result<Option<Bytes>> {
    match split_frame(&mut inbound.buffer) {
        Ok(frame) => Ok(Some(frame)),
        Err(error) if error.is_incomplete() => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn send_frame(outgoing: &mut Vec<u8>, frame: &[u8]) -> anyhow::Result<()> {
    VarInt(i32::try_from(frame.len())?).encode(&mut *outgoing)?;
    outgoing.extend_from_slice(frame);
    Ok(())
}
