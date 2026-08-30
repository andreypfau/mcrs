use crate::{EngineConnection, Instant, ReceivedPacket};
use bytes::{Bytes, BytesMut};
use log::{error, warn};
use mcrs_minecraft_protocol::{
    CompressionThreshold, Decode, Encode, Packet, PacketDecoder, PacketEncoder, WritePacket,
};
use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufWriter};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

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
    buf: BytesMut,
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
            buf: BytesMut::new(),
        }
    }

    pub fn set_compression(&mut self, threshold: CompressionThreshold) {
        self.enc.set_compression(threshold);
        self.dec.set_compression(threshold);
    }

    /// The un-typed counterpart to [`PacketIo::recv_packet`], for a phase whose
    /// next packet is not known in advance. The body is owned, so the caller
    /// can decode it and keep using `self` in the same expression.
    pub async fn recv_frame(&mut self) -> anyhow::Result<(i32, Bytes)> {
        loop {
            if let Some(frame) = self.dec.try_next_packet()? {
                return Ok((frame.id, frame.body.freeze()));
            }

            self.dec.reserve(READ_BUF_SIZE);
            let mut buf = self.dec.take_capacity();
            if self.stream.read_buf(&mut buf).await? == 0 {
                return Err(io::Error::from(ErrorKind::UnexpectedEof).into());
            }
            self.dec.queue_bytes(buf);
        }
    }

    pub async fn send_packet<P>(&mut self, pkt: &P) -> anyhow::Result<()>
    where
        P: Packet + Encode,
    {
        self.enc.append_packet(pkt)?;
        let bytes = self.enc.take();
        self.stream.write_all(&bytes).await?;
        Ok(())
    }

    pub async fn recv_packet<'a, P>(&'a mut self) -> anyhow::Result<P>
    where
        P: Packet + Decode<'a>,
    {
        loop {
            if let Some(frame) = self.dec.try_next_packet()? {
                self.buf = frame.body;
                let mut r = &self.buf[..];
                let pkt = P::decode(&mut r)?;
                return Ok(pkt);
            }

            self.dec.reserve(READ_BUF_SIZE);
            let mut buf = self.dec.take_capacity();

            if self.stream.read_buf(&mut buf).await? == 0 {
                return Err(io::Error::from(ErrorKind::UnexpectedEof).into());
            }

            // This should always be an O(1) unsplit because we reserved space earlier and
            // the call to `read_buf` shouldn't have grown the allocation.
            self.dec.queue_bytes(buf);
        }
    }

    pub fn into_raw_connection(self, remote_addr: SocketAddr) -> RawConnection
    where
        S: ByteStream,
    {
        let (incoming_sender, incoming_receiver) = mpsc::channel(256);
        let (outgoing_sender, outgoing_receiver) =
            mpsc::channel::<Bytes>(OUTBOUND_CHANNEL_CAPACITY);
        let disconnect_flag = Arc::new(AtomicBool::new(false));

        let (reader, writer) = self.stream.split_stream();
        let reader = reader_loop(reader, self.dec, incoming_sender);
        let writer = writer_loop(outgoing_receiver, writer, disconnect_flag.clone());

        #[cfg(not(target_family = "wasm"))]
        let (reader_task, writer_task) = (tokio::spawn(reader), tokio::spawn(writer));
        #[cfg(target_family = "wasm")]
        {
            wasm_bindgen_futures::spawn_local(reader);
            wasm_bindgen_futures::spawn_local(writer);
        }

        RawConnection {
            outgoing: outgoing_sender,
            recv: incoming_receiver,
            #[cfg(not(target_family = "wasm"))]
            reader_task,
            #[cfg(not(target_family = "wasm"))]
            writer_task,
            enc: self.enc,
            remote_addr,
            disconnect_flag,
        }
    }
}

async fn reader_loop<R: AsyncRead + Unpin>(
    mut reader: R,
    mut dec: PacketDecoder,
    incoming_sender: mpsc::Sender<ReceivedPacket>,
) {
    let mut buf = BytesMut::new();
    loop {
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

        let timestamp = Instant::now();

        let packet = ReceivedPacket {
            timestamp,
            id: frame.id,
            payload: frame.body.into(),
        };

        if incoming_sender.send(packet).await.is_err() {
            warn!("error sending incoming packet: receiver dropped");
            break;
        }
    }
}

async fn writer_loop<W: AsyncWrite + Unpin>(
    mut rx: mpsc::Receiver<Bytes>,
    sink: W,
    disconnect_flag: Arc<AtomicBool>,
) {
    let mut writer = BufWriter::with_capacity(64 * 1024, sink);
    while let Some(bytes) = rx.recv().await {
        if writer.write_all(&bytes).await.is_err() {
            disconnect_flag.store(true, Ordering::Relaxed);
            return;
        }
        if writer.flush().await.is_err() {
            disconnect_flag.store(true, Ordering::Relaxed);
            return;
        }
    }
    let _ = writer.flush().await;
}

pub struct RawConnection {
    outgoing: mpsc::Sender<Bytes>,
    recv: mpsc::Receiver<ReceivedPacket>,
    // A browser task cannot be aborted, so there it ends when its next read
    // resolves and the incoming channel is found closed.
    #[cfg(not(target_family = "wasm"))]
    reader_task: tokio::task::JoinHandle<()>,
    // Held to keep the writer task alive; dropped implicitly when RawConnection is dropped.
    #[cfg(not(target_family = "wasm"))]
    #[allow(dead_code)]
    writer_task: tokio::task::JoinHandle<()>,
    pub enc: PacketEncoder,
    pub remote_addr: SocketAddr,
    disconnect_flag: Arc<AtomicBool>,
}

#[cfg(not(target_family = "wasm"))]
impl Drop for RawConnection {
    fn drop(&mut self) {
        self.reader_task.abort();
        // writer shuts down naturally when outgoing sender is dropped
    }
}

impl RawConnection {
    /// Construct a mock `RawConnection` for tests that do not need real sockets.
    ///
    /// `outgoing` is the send half of a channel the test holds the receiver
    /// for; every blob passed to `try_send_blob` lands there. The dummy
    /// reader/writer tasks park immediately and are never scheduled. No TCP
    /// socket is created.
    #[cfg(not(target_family = "wasm"))]
    pub fn new_for_test(outgoing: mpsc::Sender<Bytes>) -> Self {
        let (inbound_tx, inbound_rx) = mpsc::channel::<ReceivedPacket>(32);
        let reader_task = tokio::spawn(async move {
            // Keep inbound_tx alive so the recv end never disconnects while
            // the RawConnection exists; the future parks indefinitely.
            let _keep = inbound_tx;
            std::future::pending::<()>().await;
        });
        let writer_task = tokio::spawn(async {
            std::future::pending::<()>().await;
        });
        let disconnect_flag = Arc::new(AtomicBool::new(false));
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        RawConnection {
            outgoing,
            recv: inbound_rx,
            reader_task,
            writer_task,
            enc: PacketEncoder::new(),
            remote_addr: addr,
            disconnect_flag,
        }
    }

    /// Construct a mock with separate inbound control: the caller drives
    /// the inbound channel (for bridge_inbound tests).
    ///
    /// Returns `(RawConnection, outgoing_rx, inbound_tx)`.
    #[cfg(not(target_family = "wasm"))]
    pub fn new_for_test_full(
        outgoing_capacity: usize,
    ) -> (Self, mpsc::Receiver<Bytes>, mpsc::Sender<ReceivedPacket>) {
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<Bytes>(outgoing_capacity);
        let (inbound_tx, inbound_rx) = mpsc::channel::<ReceivedPacket>(128);
        let reader_task = tokio::spawn(async {
            std::future::pending::<()>().await;
        });
        let writer_task = tokio::spawn(async {
            std::future::pending::<()>().await;
        });
        let disconnect_flag = Arc::new(AtomicBool::new(false));
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let raw = RawConnection {
            outgoing: outgoing_tx,
            recv: inbound_rx,
            reader_task,
            writer_task,
            enc: PacketEncoder::new(),
            remote_addr: addr,
            disconnect_flag,
        };
        (raw, outgoing_rx, inbound_tx)
    }

    /// Returns `true` if the blob was accepted, `false` if the channel is full or closed.
    /// The `false` case is the backpressure signal consumed by the bridge dispatch system.
    pub fn try_send_blob(&self, blob: Bytes) -> bool {
        self.outgoing.try_send(blob).is_ok()
    }

    pub fn take_encoded(&mut self) -> Bytes {
        self.enc.take().freeze()
    }

    pub fn disconnected(&self) -> bool {
        self.disconnect_flag.load(Ordering::Relaxed)
    }

    pub fn append<P: Encode + Packet>(&mut self, pkt: &P) -> anyhow::Result<()> {
        self.enc.append_packet(pkt)
    }
}

impl EngineConnection for RawConnection {
    fn try_recv(&mut self) -> Result<Option<ReceivedPacket>, TryRecvError> {
        match self.recv.try_recv() {
            Ok(packet) => Ok(Some(packet)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(TryRecvError::Disconnected),
        }
    }

    fn flush(&mut self) -> anyhow::Result<()> {
        let bytes = self.enc.take();
        if bytes.is_empty() {
            return Ok(());
        }
        let blob = bytes.freeze();
        self.outgoing
            .try_send(blob)
            .map_err(|_| anyhow::anyhow!("connection closed"))
    }

    fn queued_bytes(&self) -> usize {
        // Depth is tracked in ECS via OutboundQueue; the cross-thread atomic
        // was removed (AP-03). Return 0 so the handshake/login flush path
        // still compiles without breaking the EngineConnection contract.
        0
    }
}

impl WritePacket for RawConnection {
    fn write_packet_fallible<P>(&mut self, packet: &P) -> anyhow::Result<()>
    where
        P: Encode + Packet,
    {
        self.enc.write_packet_fallible(packet)
    }

    fn write_packet_bytes(&mut self, bytes: &[u8]) {
        self.enc.write_packet_bytes(bytes)
    }
}
