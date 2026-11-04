#[cfg(not(target_family = "wasm"))]
use crate::outbound::OutboundReceiver;
use crate::outbound::{FrameWriter, OutboundBatch, WriterQueue};
use crate::shared_frames::{SharedAppend, SharedFrames};
use crate::writer_signal::WriterSignal;
#[cfg(not(target_family = "wasm"))]
use crate::writer_wakes::WriterWakes;
use crate::{Instant, ReceivedPacket, SwitchMode};
use bytes::{Bytes, BytesMut};
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
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufWriter};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::{TryRecvError, TrySendError};
use tracing::{error, warn};

/// `Send` everywhere but the browser, where a WebTransport stream is a JS
/// object bound to the single thread that made it.
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

/// A duplex byte stream that can be torn into independently owned halves, so
/// the reader and writer loops need no shared lock. TCP, a native WebTransport
/// bidirectional stream and the browser's own are the implementations.
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

    /// The un-typed counterpart to [`PacketIo::recv_packet`], for a phase whose
    /// next packet is not known in advance. The body is owned, so the caller
    /// can decode it and keep using `self` in the same expression.
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

    /// Waits for the peer to close after the write side shuts down. Dropping the stream
    /// at once can lose the last packet: TCP resets a socket with unread input, and a
    /// WebTransport session abandons unsent stream data when it closes.
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

    /// Writes a frame that a [`PacketEncoder`] already produced for this connection's compression
    /// setting.
    pub(crate) async fn send_encoded(&mut self, frame: &[u8]) -> anyhow::Result<()> {
        self.stream.write_all(frame).await?;
        Ok(())
    }

    pub async fn flush(&mut self) -> anyhow::Result<()> {
        let bytes = self.enc.take();
        self.stream.write_all(&bytes).await?;
        Ok(())
    }

    /// Refuses a frame of another id, or one with bytes past the packet, as the game's decoder
    /// does.
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
    ) -> RawConnection
    where
        S: ByteStream,
    {
        let (incoming_sender, incoming_receiver) = mpsc::channel(256);
        let (outgoing_sender, outgoing_receiver) =
            mpsc::channel::<OutboundBatch>(OUTBOUND_CHANNEL_CAPACITY);
        let signal = Arc::new(WriterSignal::default());

        let queue = WriterQueue::new(self.enc.compression());
        let (reader, writer) = self.stream.split_stream();
        let (inbound_switch, switches) = mpsc::unbounded_channel();
        let reader = reader_loop(reader, self.dec, switches, incoming_sender);
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
            #[cfg(not(target_family = "wasm"))]
            reader_task,
            remote_addr,
            switch_mode,
            announced_compression: None,
            awaits_login_acknowledgement: false,
            signal,
            inbound_switch: Some(inbound_switch),
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

/// Decodes a packet that must take up the whole body.
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
    incoming_sender: mpsc::Sender<ReceivedPacket>,
) {
    let connection = incoming_sender.clone();
    tokio::select! {
        () = reader_loop(&mut reader, dec, switches, incoming_sender) => {}
        () = connection.closed() => {}
    }
    drop(connection);
    // TCP resets a socket closed with unread input, and the reset can erase the last packet.
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
    incoming_sender: mpsc::Sender<ReceivedPacket>,
) {
    let mut buf = BytesMut::new();
    let mut switches = Some(switches);
    let mut after = None;
    loop {
        while let Some(receiver) = &mut switches {
            match receiver.try_recv() {
                Ok(InboundSwitch::Now(threshold)) => dec.set_compression(threshold),
                Ok(InboundSwitch::After {
                    packet_id,
                    threshold,
                }) => after = Some((packet_id, threshold)),
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

        let timestamp = Instant::now();

        let packet = ReceivedPacket {
            timestamp,
            id: frame.id,
            payload: frame.body,
        };

        if incoming_sender.send(packet).await.is_err() {
            warn!("error sending incoming packet: receiver dropped");
            break;
        }
    }
}

/// Frames, compresses and writes what the connection hands over, so none of that work runs on
/// the thread that produced the packets. It sleeps on `signal`, never on the channel, so the
/// connection chooses when a send wakes it.
async fn writer_loop<W: AsyncWrite + Unpin>(
    mut rx: mpsc::Receiver<OutboundBatch>,
    sink: W,
    mut frames: FrameWriter,
    signal: Arc<WriterSignal>,
) {
    let mut writer = BufWriter::with_capacity(64 * 1024, sink);
    loop {
        // Read before the channel: a close seen here comes after every send of the connection.
        let closed = signal.is_closed();
        match rx.try_recv() {
            Ok(batch) => {
                let bytes = frames.frame(batch);
                if writer.write_all(&bytes).await.is_err() {
                    signal.mark_disconnected();
                    return;
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
    recv: mpsc::Receiver<ReceivedPacket>,
    // A browser task cannot be aborted, so there it ends when its next read
    // resolves and the incoming channel is found closed.
    #[cfg(not(target_family = "wasm"))]
    reader_task: tokio::task::JoinHandle<()>,
    pub remote_addr: SocketAddr,
    switch_mode: SwitchMode,
    announced_compression: Option<CompressionThreshold>,
    awaits_login_acknowledgement: bool,
    /// What was written and not yet taken by the writer, sent ahead of anything newer: a slow
    /// socket delays what it is sent and never loses it.
    queue: WriterQueue,
    signal: Arc<WriterSignal>,
    inbound_switch: Option<mpsc::UnboundedSender<InboundSwitch>>,
}

/// What one hand-over to the writer did.
struct HandOver {
    /// At least one batch went into the channel, so the writer has something to wake for.
    sent: bool,
    /// Every batch went; `false` when the writer is behind or gone.
    room: bool,
}

/// The reader task splits frames ahead of the systems that act on them, so it applies a
/// compression switch itself, before it splits the next frame.
enum InboundSwitch {
    Now(CompressionThreshold),
    /// Switches right after the next frame with this packet id, which can share a read with
    /// the compressed frames that follow it.
    After {
        packet_id: i32,
        threshold: CompressionThreshold,
    },
}

impl Drop for RawConnection {
    fn drop(&mut self) {
        #[cfg(not(target_family = "wasm"))]
        self.reader_task.abort();
        self.signal.close();
    }
}

impl RawConnection {
    /// Construct a mock with separate inbound control: the caller drives
    /// the inbound channel (for bridge_inbound tests).
    ///
    /// Returns `(RawConnection, outgoing_rx, inbound_tx)`.
    #[cfg(not(target_family = "wasm"))]
    pub fn new_for_test_full(
        outgoing_capacity: usize,
    ) -> (Self, OutboundReceiver, mpsc::Sender<ReceivedPacket>) {
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<OutboundBatch>(outgoing_capacity);
        let (inbound_tx, inbound_rx) = mpsc::channel::<ReceivedPacket>(128);
        let reader_task = tokio::spawn(async {
            std::future::pending::<()>().await;
        });
        let addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let raw = RawConnection {
            outgoing: outgoing_tx,
            recv: inbound_rx,
            reader_task,
            remote_addr: addr,
            switch_mode: SwitchMode::of_own_version(),
            announced_compression: None,
            awaits_login_acknowledgement: false,
            signal: Arc::new(WriterSignal::default()),
            inbound_switch: Some(mpsc::unbounded_channel().0),
            queue: WriterQueue::default(),
        };
        (raw, OutboundReceiver::new(outgoing_rx), inbound_tx)
    }

    /// Hands everything written so far to the writer, behind anything still waiting. Answers
    /// `false` when the writer is behind, which is the backpressure signal the bridge reads;
    /// what it had no room for is kept.
    pub fn send_written(&mut self) -> bool {
        self.queue.close();
        self.flush_unsent()
    }

    /// Sends what is waiting, in order, as far as the writer has room.
    pub fn flush_unsent(&mut self) -> bool {
        let handed = self.send_unsent();
        if handed.sent {
            self.signal.wake();
        }
        handed.room
    }

    /// [`Self::send_written`], with the wake of the writer left to [`WriterWakes::wake_all`].
    #[cfg(not(target_family = "wasm"))]
    pub fn send_written_deferred(&mut self, wakes: &mut WriterWakes) -> bool {
        self.queue.close();
        self.flush_unsent_deferred(wakes)
    }

    /// [`Self::flush_unsent`], with the wake of the writer left to [`WriterWakes::wake_all`].
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
                Ok(()) => sent = true,
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

    /// Writes the compression packet of LOGIN. In the immediate mode both directions switch
    /// here; the client sends nothing between its hello and its first compressed frame.
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

    /// Closes the switch channel too: no switch follows `login_finished`, so the reader stops
    /// polling for one.
    pub fn finish_login(&mut self, profile: GameProfile<'_>, session_id: Uuid) {
        let switches = self.inbound_switch.take();
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
                if let Some(threshold) = self.announced_compression
                    && let Some(switches) = switches
                {
                    let _ = switches.send(InboundSwitch::After {
                        packet_id: ServerboundLoginAcknowledged::ID,
                        threshold,
                    });
                }
                self.awaits_login_acknowledgement = true;
            }
        }
    }

    /// Call once the `login_acknowledged` that answers `login_finished` is accepted.
    pub fn login_acknowledged(&mut self) {
        self.awaits_login_acknowledgement = false;
        if self.switch_mode == SwitchMode::Acknowledged
            && let Some(threshold) = self.announced_compression
        {
            self.queue.switch_compression(threshold);
        }
    }

    /// In the acknowledged mode, between `login_finished` and its acknowledgement, the client
    /// decompresses what it reads and the server does not yet compress what it writes, so no
    /// frame written then can be read.
    pub fn awaits_login_acknowledgement(&self) -> bool {
        self.awaits_login_acknowledgement
    }

    /// Sets the mode of a connection made by [`Self::new_for_test_full`], before its login.
    #[cfg(not(target_family = "wasm"))]
    pub fn with_switch_mode(mut self, switch_mode: SwitchMode) -> Self {
        self.switch_mode = switch_mode;
        self
    }

    pub fn disconnected(&self) -> bool {
        self.signal.disconnected()
    }

    /// Serializes the packet; the writer frames and compresses it.
    pub fn append<P: Encode + Packet>(&mut self, pkt: &P) -> anyhow::Result<()> {
        self.queue.append(pkt)
    }

    /// Bytes of packet bodies, with ids and before framing and compression, appended since the
    /// connection was made.
    pub fn appended_bytes(&self) -> u64 {
        self.queue.appended_bytes()
    }

    /// Queues `records` without a copy: packets with their ids, each behind its length as a
    /// big-endian `u32`, which other connections may share. The writer frames each packet. Returns
    /// how many packets `records` holds; it ends at a length that runs past its end.
    pub fn append_records(&mut self, records: Bytes) -> usize {
        self.queue.append_records(records)
    }

    /// Queues a packet that `encode_with_id` wrote once for every connection that receives it,
    /// without a copy. A packet that `frames` counted for more than one connection goes as the
    /// frame that `frames` makes once for this connection's threshold, so it is compressed once
    /// for all of them.
    pub fn append_shared(
        &mut self,
        packet: Bytes,
        frames: &mut SharedFrames,
    ) -> anyhow::Result<SharedAppend> {
        let Some(found) = frames.frame(&packet, self.queue.compression())? else {
            self.queue.append_shared(packet);
            return Ok(SharedAppend::Unframed);
        };
        self.queue.append_framed(found.frame, packet.len());
        Ok(found.append)
    }

    /// True once the reader has stopped, even while frames it read are still queued.
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

    /// Hands everything written so far to the writer, keeping what it has no room for yet.
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
    use std::time::Duration;

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
            PacketIo::new(stream).into_raw_connection(remote, switch_mode),
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
    fn a_deferred_wake_delivers_the_batch_and_a_dropped_connection_then_closes_the_socket() {
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
    fn an_acknowledged_login_reads_compressed_frames_from_right_after_login_acknowledged() {
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
}
