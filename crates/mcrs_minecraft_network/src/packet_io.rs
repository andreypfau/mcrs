use crate::inbound;
use crate::inbound::InboundReceiver;
use crate::inbound::InboundSender;
#[cfg(not(target_family = "wasm"))]
use crate::outbound::OutboundReceiver;
use crate::outbound::{FrameWriter, OutboundBatch, WriterQueue};
use crate::shared_frames::{SharedAppend, SharedFrames};
use crate::writer_signal::WriterSignal;
#[cfg(not(target_family = "wasm"))]
use crate::writer_wakes::WriterWakes;
use crate::{Instant, ReceivedPacket, SwitchMode};
use bytes::{Bytes, BytesMut};
use mcrs_minecraft_move_validate::{Verdict, check_serverbound};
use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundFinishConfiguration;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundConfigurationAcknowledged;
use mcrs_minecraft_protocol::packets::login::clientbound::{
    ClientboundLoginFinished, ClientboundLoginFinishedWithOptions, LoginCompression, OptionFlags,
};
use mcrs_minecraft_protocol::packets::login::serverbound::ServerboundLoginAcknowledged;
use mcrs_minecraft_protocol::profile::GameProfile;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{
    CompressionThreshold, Decode, Encode, Packet, PacketDecoder, PacketEncoder, VarInt, WritePacket,
};
use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;
#[cfg(not(target_family = "wasm"))]
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufWriter};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::{TryRecvError, TrySendError};
use tracing::{error, warn};

#[cfg(not(target_family = "wasm"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_family = "wasm"))]
impl<T: Send> MaybeSend for T {}
#[cfg(target_family = "wasm")]
pub trait MaybeSend {}
#[cfg(target_family = "wasm")]
impl<T> MaybeSend for T {}

#[cfg(not(target_family = "wasm"))]
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

pub trait ByteStream: AsyncRead + AsyncWrite + Unpin + MaybeSend + 'static {
    type Reader: AsyncRead + Unpin + MaybeSend + 'static;
    type Writer: AsyncWrite + Unpin + MaybeSend + 'static;

    fn split_stream(self) -> (Self::Reader, Self::Writer);
}

#[cfg(not(target_family = "wasm"))]
impl ByteStream for tokio::net::TcpStream {
    type Reader = tokio::net::tcp::OwnedReadHalf;
    type Writer = tokio::net::tcp::OwnedWriteHalf;

    fn split_stream(self) -> (Self::Reader, Self::Writer) {
        self.into_split()
    }
}

pub struct PacketIo<S> {
    stream: S,
    enc: PacketEncoder,
    dec: PacketDecoder,
    buf: Bytes,
}

const READ_BUF_SIZE: usize = 4096;

pub(crate) const OUTBOUND_CHANNEL_CAPACITY: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inbound {
    FromClient,
    FromServer,
}

impl Inbound {
    fn capacity(self) -> usize {
        match self {
            Self::FromClient => 256,
            Self::FromServer => 65_536,
        }
    }

    fn payload_budget(self) -> usize {
        match self {
            Self::FromClient => 4 * 1024 * 1024,
            Self::FromServer => 64 * 1024 * 1024,
        }
    }
}
pub const MAX_QUEUED_BYTES_PER_SOCKET: usize = 4 * 1024 * 1024;

#[cfg(not(target_family = "wasm"))]
impl PacketIo<tokio::net::TcpStream> {
    pub async fn connect(server: SocketAddr) -> io::Result<Self> {
        let stream = tokio::net::TcpStream::connect(server).await?;
        stream.set_nodelay(true)?;
        Ok(Self::new(stream))
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> PacketIo<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            enc: PacketEncoder::new(),
            dec: PacketDecoder::new(),
            buf: Bytes::new(),
        }
    }

    pub fn set_compression(&mut self, threshold: CompressionThreshold) {
        self.set_inbound_compression(threshold);
        self.set_outbound_compression(threshold);
    }

    pub fn set_inbound_compression(&mut self, threshold: CompressionThreshold) {
        self.dec.set_compression(threshold);
    }

    pub fn set_outbound_compression(&mut self, threshold: CompressionThreshold) {
        self.enc.set_compression(threshold);
    }

    pub async fn recv_frame(&mut self) -> anyhow::Result<(i32, Bytes)> {
        loop {
            if let Some(frame) = self.dec.try_next_packet()? {
                return Ok((frame.id, frame.body));
            }

            self.dec.reserve(READ_BUF_SIZE);
            let mut buf = self.dec.take_capacity();
            if self.stream.read_buf(&mut buf).await? == 0 {
                return Err(io::Error::from(ErrorKind::UnexpectedEof).into());
            }
            self.dec.queue_bytes(buf);
        }
    }

    pub async fn close(mut self) -> io::Result<()> {
        self.stream.shutdown().await?;
        let mut scratch = [0; READ_BUF_SIZE];
        while self.stream.read(&mut scratch).await? != 0 {}
        Ok(())
    }

    pub async fn send_packet<P>(&mut self, pkt: &P) -> anyhow::Result<()>
    where
        P: Packet + Encode,
    {
        self.enc.append_packet(pkt)?;
        self.flush().await
    }

    pub(crate) async fn send_encoded(&mut self, frame: &[u8]) -> anyhow::Result<()> {
        self.stream.write_all(frame).await?;
        Ok(())
    }

    pub async fn flush(&mut self) -> anyhow::Result<()> {
        let bytes = self.enc.take();
        self.stream.write_all(&bytes).await?;
        Ok(())
    }

    pub async fn recv_packet<'a, P>(&'a mut self) -> anyhow::Result<P>
    where
        P: Packet + Decode<'a>,
    {
        let (id, body) = self.recv_frame().await?;
        anyhow::ensure!(
            id == P::ID,
            "expected {} (id {}), received id {id}",
            P::NAME,
            P::ID
        );
        self.buf = body;
        decode_body(&self.buf)
    }

    pub fn into_raw_connection(
        self,
        remote_addr: SocketAddr,
        switch_mode: SwitchMode,
        inbound: Inbound,
    ) -> RawConnection
    where
        S: ByteStream,
    {
        let (incoming_sender, incoming_receiver) =
            inbound::channel(inbound.capacity(), inbound.payload_budget());
        let (outgoing_sender, outgoing_receiver) =
            mpsc::channel::<OutboundBatch>(OUTBOUND_CHANNEL_CAPACITY);
        let signal = Arc::new(WriterSignal::default());

        let queue = WriterQueue::new(self.enc.compression());
        let (reader, writer) = self.stream.split_stream();
        let (inbound_switch, switches) = mpsc::unbounded_channel();
        let writer = writer_loop(
            outgoing_receiver,
            writer,
            FrameWriter::new(self.enc),
            Arc::clone(&signal),
        );

        #[cfg(not(target_family = "wasm"))]
        {
            tokio::spawn(read_until_closed(
                reader,
                self.dec,
                switches,
                incoming_sender,
            ));
            tokio::spawn(writer);
        }
        #[cfg(target_family = "wasm")]
        {
            wasm_bindgen_futures::spawn_local(reader_loop(
                reader,
                self.dec,
                switches,
                incoming_sender,
            ));
            wasm_bindgen_futures::spawn_local(writer);
        }

        RawConnection {
            outgoing: outgoing_sender,
            recv: incoming_receiver,
            remote_addr,
            switch_mode,
            announced_compression: None,
            awaits_login_acknowledgement: false,
            signal,
            handed_at: Instant::now(),
            inbound_switch: Some(inbound_switch),
            play_switch: None,
            queue,
        }
    }
}

impl<S> WritePacket for PacketIo<S> {
    fn write_packet_fallible<P>(&mut self, packet: &P) -> anyhow::Result<()>
    where
        P: Encode + Packet,
    {
        self.enc.append_packet(packet)
    }
}

pub(crate) fn decode_body<'a, P: Decode<'a> + Packet>(body: &'a [u8]) -> anyhow::Result<P> {
    let mut r = body;
    let packet =
        anyhow::Context::with_context(P::decode(&mut r), || format!("reading {}", P::NAME))?;
    anyhow::ensure!(r.is_empty(), "{} bytes left over in {}", r.len(), P::NAME);
    Ok(packet)
}

#[cfg(not(target_family = "wasm"))]
async fn read_until_closed<R: AsyncRead + Unpin>(
    mut reader: R,
    dec: PacketDecoder,
    switches: mpsc::UnboundedReceiver<InboundSwitch>,
    incoming_sender: InboundSender,
) {
    let connection = incoming_sender.packets.clone();
    tokio::select! {
        () = reader_loop(&mut reader, dec, switches, incoming_sender) => {}
        () = connection.closed() => {}
    }
    drop(connection);
    let drain = async {
        let mut scratch = [0; READ_BUF_SIZE];
        while matches!(reader.read(&mut scratch).await, Ok(n) if n != 0) {}
    };
    let _ = tokio::time::timeout(CLOSE_TIMEOUT, drain).await;
}

async fn reader_loop<R: AsyncRead + Unpin>(
    mut reader: R,
    mut dec: PacketDecoder,
    switches: mpsc::UnboundedReceiver<InboundSwitch>,
    mut incoming_sender: InboundSender,
) {
    let mut buf = BytesMut::new();
    let mut switches = Some(switches);
    let mut after = None;
    let mut play = PlayChecks::Off;
    let mut last: Option<Instant> = None;
    loop {
        while let Some(receiver) = &mut switches {
            match receiver.try_recv() {
                Ok(InboundSwitch::Now(threshold)) => dec.set_compression(threshold),
                Ok(InboundSwitch::After {
                    packet_id,
                    threshold,
                }) => after = Some((packet_id, threshold)),
                Ok(InboundSwitch::CheckPlay) => play = PlayChecks::AfterConfiguration,
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => switches = None,
            }
        }
        let frame = match dec.try_next_packet() {
            Ok(Some(frame)) => frame,
            Ok(None) => {
                buf.reserve(READ_BUF_SIZE);
                match reader.read_buf(&mut buf).await {
                    Ok(0) => {
                        warn!("Connection closed!");
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("error reading data from stream: {e}");
                        break;
                    }
                }
                dec.queue_bytes(buf.split());
                continue;
            }
            Err(e) => {
                warn!("error decoding packet: {e}");
                break;
            }
        };

        if let Some((packet_id, threshold)) = after
            && frame.id == packet_id
        {
            dec.set_compression(threshold);
            after = None;
        }

        let now = Instant::now();
        let timestamp = last
            .and_then(|last| last.checked_add(Duration::from_nanos(1)))
            .map_or(now, |next| next.max(now));
        last = Some(timestamp);
        let verdict = play.check(frame.id, &frame.body);
        play = play.after(frame.id);
        let packet = ReceivedPacket {
            timestamp,
            id: frame.id,
            payload: frame.body,
            verdict,
        };

        if incoming_sender.send(packet).await.is_err() {
            warn!("error sending incoming packet: receiver dropped");
            break;
        }
    }
}

async fn writer_loop<W: AsyncWrite + Unpin>(
    mut rx: mpsc::Receiver<OutboundBatch>,
    sink: W,
    mut frames: FrameWriter,
    signal: Arc<WriterSignal>,
) {
    let mut writer = BufWriter::with_capacity(64 * 1024, sink);
    let mut chunks = Vec::new();
    loop {
        let closed = signal.is_closed();
        match rx.try_recv() {
            Ok(batch) => {
                frames.frame_into(batch, &mut chunks);
                for chunk in chunks.drain(..) {
                    if writer.write_all(&chunk).await.is_err() {
                        signal.mark_disconnected();
                        return;
                    }
                }
            }
            Err(TryRecvError::Empty) => {
                if writer.flush().await.is_err() {
                    signal.mark_disconnected();
                    return;
                }
                if closed {
                    return;
                }
                signal.woken().await;
            }
            Err(TryRecvError::Disconnected) => break,
        }
    }
    let _ = writer.flush().await;
}

pub struct RawConnection {
    outgoing: mpsc::Sender<OutboundBatch>,
    recv: InboundReceiver,
    pub remote_addr: SocketAddr,
    switch_mode: SwitchMode,
    announced_compression: Option<CompressionThreshold>,
    awaits_login_acknowledgement: bool,
    queue: WriterQueue,
    handed_at: Instant,
    signal: Arc<WriterSignal>,
    inbound_switch: Option<mpsc::UnboundedSender<InboundSwitch>>,
    play_switch: Option<mpsc::UnboundedSender<InboundSwitch>>,
}

struct HandOver {
    sent: bool,
    room: bool,
}

enum InboundSwitch {
    Now(CompressionThreshold),
    After {
        packet_id: i32,
        threshold: CompressionThreshold,
    },
    CheckPlay,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlayChecks {
    Off,
    AfterConfiguration,
    On,
}

impl PlayChecks {
    fn check(self, frame_id: i32, body: &[u8]) -> Option<Verdict> {
        (self == Self::On).then(|| check_serverbound(frame_id, body))
    }

    fn after(self, frame_id: i32) -> Self {
        match self {
            Self::AfterConfiguration if frame_id == ServerboundFinishConfiguration::ID => Self::On,
            Self::On if frame_id == ServerboundConfigurationAcknowledged::ID => Self::Off,
            other => other,
        }
    }
}

impl Drop for RawConnection {
    fn drop(&mut self) {
        self.signal.close();
    }
}

impl RawConnection {
    #[cfg(not(target_family = "wasm"))]
    pub fn new_for_test_full(
        outgoing_capacity: usize,
    ) -> (Self, OutboundReceiver, mpsc::Sender<ReceivedPacket>) {
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<OutboundBatch>(outgoing_capacity);
        let (inbound_tx, inbound_rx) = mpsc::channel::<ReceivedPacket>(128);
        let addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let raw = RawConnection {
            outgoing: outgoing_tx,
            recv: InboundReceiver::from_packets(inbound_rx),
            remote_addr: addr,
            switch_mode: SwitchMode::of_own_version(),
            announced_compression: None,
            awaits_login_acknowledgement: false,
            signal: Arc::new(WriterSignal::default()),
            handed_at: Instant::now(),
            inbound_switch: Some(mpsc::unbounded_channel().0),
            play_switch: None,
            queue: WriterQueue::default(),
        };
        (raw, OutboundReceiver::new(outgoing_rx), inbound_tx)
    }

    pub fn send_written(&mut self) -> bool {
        self.queue.close();
        let handed = self.send_unsent();
        if handed.sent {
            self.signal.wake();
        }
        handed.room
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn send_written_deferred(&mut self, wakes: &mut WriterWakes) -> bool {
        self.queue.close();
        self.flush_unsent_deferred(wakes)
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn flush_unsent_deferred(&mut self, wakes: &mut WriterWakes) -> bool {
        let handed = self.send_unsent();
        if handed.sent {
            wakes.push(&self.signal);
        }
        handed.room
    }

    fn send_unsent(&mut self) -> HandOver {
        let mut sent = false;
        while let Some(batch) = self.queue.pop() {
            match self.outgoing.try_send(batch) {
                Ok(()) => {
                    sent = true;
                    self.handed_at = Instant::now();
                }
                Err(TrySendError::Full(batch)) => {
                    self.queue.push_front(batch);
                    return HandOver { sent, room: false };
                }
                Err(TrySendError::Closed(_)) => return HandOver { sent, room: false },
            }
        }
        HandOver { sent, room: true }
    }

    pub fn unsent_bytes(&self) -> usize {
        self.queue.closed_bytes()
    }

    pub fn since_handover(&self) -> std::time::Duration {
        self.handed_at.elapsed()
    }

    pub fn announce_compression(&mut self, threshold: CompressionThreshold) {
        let Some(switches) = &self.inbound_switch else {
            warn!("compression announced after login_finished; ignored");
            return;
        };
        if self.switch_mode == SwitchMode::Immediate {
            let _ = switches.send(InboundSwitch::Now(threshold));
        }
        self.write_packet(&LoginCompression {
            threshold: VarInt(threshold.0),
        });
        self.announced_compression = Some(threshold);
        if self.switch_mode == SwitchMode::Immediate {
            self.queue.switch_compression(threshold);
        }
    }

    pub fn finish_login(&mut self, profile: GameProfile<'_>, session_id: Uuid) {
        let switches = self.inbound_switch.take();
        self.play_switch = switches.clone();
        match self.switch_mode {
            SwitchMode::Immediate => {
                self.write_packet(&ClientboundLoginFinished {
                    profile,
                    session_id,
                });
            }
            SwitchMode::Acknowledged => {
                self.write_packet(&ClientboundLoginFinishedWithOptions {
                    profile,
                    session_id,
                    options: OptionFlags::new()
                        .with_compression(self.announced_compression.is_some()),
                });
                if let Some(threshold) = self.announced_compression {
                    self.queue.switch_compression(threshold);
                    if let Some(switches) = switches {
                        let _ = switches.send(InboundSwitch::After {
                            packet_id: ServerboundLoginAcknowledged::ID,
                            threshold,
                        });
                    }
                }
                self.awaits_login_acknowledgement = true;
            }
        }
    }

    pub fn check_play_moves(&mut self) {
        if let Some(switches) = &self.play_switch {
            let _ = switches.send(InboundSwitch::CheckPlay);
        }
    }

    pub fn login_acknowledged(&mut self) {
        self.awaits_login_acknowledgement = false;
    }

    pub fn awaits_login_acknowledgement(&self) -> bool {
        self.awaits_login_acknowledgement
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn with_switch_mode(mut self, switch_mode: SwitchMode) -> Self {
        self.switch_mode = switch_mode;
        self
    }

    pub fn disconnected(&self) -> bool {
        self.signal.disconnected()
    }

    pub fn append<P: Encode + Packet>(&mut self, pkt: &P) -> anyhow::Result<()> {
        self.queue.append(pkt)
    }

    pub fn append_records(&mut self, records: Bytes) -> usize {
        self.queue.append_records(records)
    }

    pub fn append_deferred(&mut self, records: Bytes) -> usize {
        self.queue.append_deferred(records)
    }

    pub fn fence(&mut self) {
        self.queue.fence();
    }

    pub fn append_shared(
        &mut self,
        packet: Bytes,
        frames: &mut SharedFrames,
    ) -> anyhow::Result<SharedAppend> {
        let Some(found) = frames.frame(&packet, self.queue.compression())? else {
            self.queue.append_shared(packet);
            return Ok(SharedAppend::Unframed);
        };
        self.queue.append_framed(found.frame);
        Ok(found.append)
    }

    pub fn reader_closed(&self) -> bool {
        self.recv.is_closed()
    }

    pub fn try_recv(&mut self) -> Result<Option<ReceivedPacket>, TryRecvError> {
        match self.recv.try_recv() {
            Ok(packet) => Ok(Some(packet)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(TryRecvError::Disconnected),
        }
    }

    pub fn flush(&mut self) -> anyhow::Result<()> {
        self.send_written();
        anyhow::ensure!(!self.outgoing.is_closed(), "connection closed");
        Ok(())
    }
}

impl WritePacket for RawConnection {
    fn write_packet_fallible<P>(&mut self, packet: &P) -> anyhow::Result<()>
    where
        P: Encode + Packet,
    {
        self.queue.append(packet)
    }
}

#[cfg(test)]
#[cfg(not(target_family = "wasm"))]
mod tests {
    use super::*;
    use mcrs_minecraft_protocol::VarInt;
    use mcrs_minecraft_protocol::packets::login::clientbound::LoginCompression;
    use mcrs_minecraft_protocol::packets::ping::serverbound::PingRequest;
    use mcrs_minecraft_protocol::packets::status::clientbound::StatusResponse;
    use std::future::Future;
    use std::task::Context;
    use std::task::Poll;
    use std::task::Waker;
    use std::time::Duration;

    #[test]
    fn compressed_inbound_packets_wait_for_payload_budget_and_resume_in_order() {
        let threshold = CompressionThreshold(1);
        let mut encoder = PacketEncoder::new();
        encoder.set_compression(threshold);
        for id in 1..=3 {
            let packet = vec![id; 3 * 1024 * 1024];
            encoder.append_encoded(&packet).unwrap();
        }
        let wire = encoder.take();
        let mut decoder = PacketDecoder::new();
        decoder.set_compression(threshold);
        let (_switch, switches) = mpsc::unbounded_channel();
        let inbound = Inbound::FromClient;
        let (sender, mut receiver) = inbound::channel(inbound.capacity(), inbound.payload_budget());
        let mut reading = Box::pin(reader_loop(&wire[..], decoder, switches, sender));
        let mut context = Context::from_waker(Waker::noop());

        for expected in 1..=3 {
            let progress = if expected < 3 {
                Poll::Pending
            } else {
                Poll::Ready(())
            };
            assert_eq!(reading.as_mut().poll(&mut context), progress);
            let packet = receiver.try_recv().unwrap();
            assert_eq!(packet.id, expected);
            assert_eq!(packet.payload.len(), 3 * 1024 * 1024 - 1);
            assert!(
                receiver.try_recv().is_err(),
                "only one large packet fits the client budget"
            );
        }
    }

    fn body_of<P: Encode>(packet: &P) -> Vec<u8> {
        let mut body = Vec::new();
        packet.encode(&mut body).unwrap();
        body
    }

    #[test]
    fn packets_cross_a_duplex_stream_with_the_threshold_set_between_two() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (near, far) = tokio::io::duplex(64 * 1024);
            let mut writer = PacketIo::new(near);
            let mut reader = PacketIo::new(far);
            let json = "a".repeat(1000);
            let ping = PingRequest { payload: 7 };
            let status = StatusResponse { json: &json };

            tokio::time::timeout(Duration::from_secs(10), async {
                writer
                    .send_packet(&LoginCompression {
                        threshold: VarInt(256),
                    })
                    .await
                    .unwrap();
                writer.set_compression(CompressionThreshold(256));
                writer.send_packet(&ping).await.unwrap();
                writer.send_packet(&status).await.unwrap();

                let (id, frame) = reader.recv_frame().await.unwrap();
                assert_eq!(id, LoginCompression::ID);
                let compression = LoginCompression::decode(&mut &frame[..]).unwrap();
                reader.set_compression(CompressionThreshold(compression.threshold.0));

                let (id, frame) = reader.recv_frame().await.unwrap();
                assert_eq!(id, PingRequest::ID);
                assert_eq!(&frame[..], &body_of(&ping)[..]);

                let (id, frame) = reader.recv_frame().await.unwrap();
                assert_eq!(id, StatusResponse::ID);
                assert_eq!(&frame[..], &body_of(&status)[..]);
            })
            .await
            .unwrap();
        });
    }

    async fn loopback(switch_mode: SwitchMode) -> (RawConnection, tokio::net::TcpStream) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer = tokio::net::TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (stream, remote) = listener.accept().await.unwrap();
        (
            PacketIo::new(stream).into_raw_connection(remote, switch_mode, Inbound::FromClient),
            peer,
        )
    }

    async fn next_received(connection: &mut RawConnection) -> ReceivedPacket {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(packet) = connection.try_recv().unwrap() {
                    return packet;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap()
    }

    #[test]
    fn outbound_frames_at_or_above_the_threshold_are_compressed_and_smaller_ones_plain() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (mut connection, mut peer) = loopback(SwitchMode::Immediate).await;
            let json = "a".repeat(1000);
            let status = StatusResponse { json: &json };
            connection.announce_compression(CompressionThreshold(256));
            connection.append(&PingRequest { payload: 7 }).unwrap();
            connection.append(&status).unwrap();
            connection.flush().unwrap();

            let mut wire = BytesMut::new();
            let mut frames = Vec::new();
            tokio::time::timeout(Duration::from_secs(10), async {
                while frames.len() < 3 {
                    match mcrs_minecraft_protocol::frame::split_frame(&mut wire) {
                        Ok(frame) => frames.push(frame),
                        Err(error) if error.is_incomplete() => {
                            peer.read_buf(&mut wire).await.unwrap();
                        }
                        Err(error) => panic!("{error}"),
                    }
                }
            })
            .await
            .unwrap();
            let leading_var_int = |frame: &Bytes| VarInt::decode(&mut &frame[..]).unwrap().0;
            assert_eq!(leading_var_int(&frames[0]), LoginCompression::ID);
            assert_eq!(leading_var_int(&frames[1]), 0);
            let status_length = 1 + body_of(&status).len();
            assert_eq!(leading_var_int(&frames[2]) as usize, status_length);
            assert!(frames[2].len() < status_length);
        });
    }

    #[test]
    fn deferred_wake_delivers_batch_and_dropped_connection_closes_socket() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut wakes = WriterWakes::on_runtime(runtime.handle().clone());
        let (mut connection, mut peer) = runtime.block_on(loopback(SwitchMode::Immediate));
        let ping = PingRequest { payload: 7 };
        connection.append(&ping).unwrap();
        assert!(connection.send_written_deferred(&mut wakes));
        wakes.wake_all();
        let mut expected = PacketEncoder::new();
        expected.append_packet(&ping).unwrap();
        let expected = expected.take();

        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(10), async {
                let mut frame = vec![0; expected.len()];
                peer.read_exact(&mut frame).await.unwrap();
                assert_eq!(frame, &expected[..]);
                drop(connection);
                let mut rest = Vec::new();
                peer.read_to_end(&mut rest).await.unwrap();
                assert!(rest.is_empty(), "nothing follows the one packet");
            })
            .await
            .unwrap();
        });
    }

    #[test]
    fn peer_sending_after_drop_reads_last_packet_then_clean_close() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (mut connection, mut peer) = loopback(SwitchMode::Immediate).await;
            connection.append(&PingRequest { payload: 7 }).unwrap();
            connection.flush().unwrap();
            drop(connection);
            tokio::time::sleep(Duration::from_millis(100)).await;

            peer.write_all(&[0; 64]).await.unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;

            let mut received = Vec::new();
            let result =
                tokio::time::timeout(Duration::from_secs(10), peer.read_to_end(&mut received))
                    .await
                    .unwrap();
            assert!(result.is_ok(), "{result:?}");
            assert!(
                !received.is_empty(),
                "the packet written before the drop arrives"
            );
        });
    }

    #[test]
    fn acked_login_reads_compressed_frames_after_login_acknowledged() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (mut connection, mut peer) = loopback(SwitchMode::Acknowledged).await;
            connection.announce_compression(CompressionThreshold(256));
            connection.finish_login(
                GameProfile {
                    id: Uuid::nil(),
                    username: mcrs_minecraft_protocol::Bounded("Steve"),
                    properties: Default::default(),
                },
                Uuid::nil(),
            );
            let json = "a".repeat(1000);
            let status = StatusResponse { json: &json };

            let mut enc = PacketEncoder::new();
            enc.append_packet(&ServerboundLoginAcknowledged).unwrap();
            enc.set_compression(CompressionThreshold(256));
            enc.append_packet(&status).unwrap();
            peer.write_all(&enc.take()).await.unwrap();

            let first = next_received(&mut connection).await;
            assert_eq!(first.id, ServerboundLoginAcknowledged::ID);
            let second = next_received(&mut connection).await;
            assert_eq!(
                (second.id, &second.payload[..]),
                (StatusResponse::ID, &body_of(&status)[..])
            );
        });
    }

    #[test]
    fn acked_login_compresses_frames_after_login_finished() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (mut connection, mut peer) = loopback(SwitchMode::Acknowledged).await;
            connection.announce_compression(CompressionThreshold(256));
            connection.finish_login(
                GameProfile {
                    id: Uuid::nil(),
                    username: mcrs_minecraft_protocol::Bounded("Steve"),
                    properties: Default::default(),
                },
                Uuid::nil(),
            );
            connection.append(&PingRequest { payload: 7 }).unwrap();
            connection.flush().unwrap();

            let mut wire = BytesMut::new();
            let mut frames = Vec::new();
            tokio::time::timeout(Duration::from_secs(10), async {
                while frames.len() < 3 {
                    match mcrs_minecraft_protocol::frame::split_frame(&mut wire) {
                        Ok(frame) => frames.push(frame),
                        Err(error) if error.is_incomplete() => {
                            peer.read_buf(&mut wire).await.unwrap();
                        }
                        Err(error) => panic!("{error}"),
                    }
                }
            })
            .await
            .unwrap();
            let leading_var_int = |frame: &Bytes| VarInt::decode(&mut &frame[..]).unwrap().0;
            assert_eq!(
                leading_var_int(&frames[2]),
                0,
                "a small frame in the compressed format, with no data length"
            );
        });
    }

    #[test]
    fn the_reader_checks_moves_only_in_the_play_state() {
        let move_player_pos = mcrs_minecraft_protocol::packets::table::row_of(
            mcrs_minecraft_protocol::packets::table::game_serverbound::NAMES,
            "move_player_pos",
        )
        .unwrap();
        let not_finite: Vec<u8> = [f64::NAN, 0.0, 0.0]
            .iter()
            .flat_map(|value| value.to_be_bytes())
            .chain([0])
            .collect();
        let frame = |id: i32, body: &[u8]| {
            let mut frame = vec![u8::try_from(body.len() + 1).unwrap(), id as u8];
            frame.extend_from_slice(body);
            frame
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (mut connection, mut peer) = loopback(SwitchMode::Immediate).await;
            connection.finish_login(
                GameProfile {
                    id: Uuid::nil(),
                    username: mcrs_minecraft_protocol::Bounded("Steve"),
                    properties: Default::default(),
                },
                Uuid::nil(),
            );
            connection.check_play_moves();
            let wire = [
                frame(move_player_pos, &not_finite),
                frame(ServerboundFinishConfiguration::ID, &[]),
                frame(move_player_pos, &not_finite),
                frame(ServerboundConfigurationAcknowledged::ID, &[]),
                frame(move_player_pos, &not_finite),
            ]
            .concat();
            peer.write_all(&wire).await.unwrap();

            let mut verdicts = Vec::new();
            for _ in 0..5 {
                verdicts.push(next_received(&mut connection).await.verdict);
            }
            let refused = Some(Verdict::Disconnect(
                mcrs_minecraft_move_validate::Disconnect::InvalidPlayerMovement,
            ));
            assert_eq!(verdicts[..3], [None, None, refused]);
            assert!(matches!(verdicts[3], Some(Verdict::Pass(_))));
            assert_eq!(verdicts[4], None);
        });
    }
}
