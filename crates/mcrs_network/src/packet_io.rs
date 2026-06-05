use crate::{EngineConnection, ReceivedPacket};
use bytes::{Bytes, BytesMut};
use log::{error, warn};
use mcrs_protocol::{Decode, Encode, Packet, PacketDecoder, PacketEncoder, WritePacket};
use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::task::JoinHandle;

pub(crate) struct PacketIo {
    stream: tokio::net::TcpStream,
    enc: PacketEncoder,
    dec: PacketDecoder,
    buf: BytesMut,
}

const READ_BUF_SIZE: usize = 4096;

pub(crate) const OUTBOUND_CHANNEL_CAPACITY: usize = 4;
pub const MAX_QUEUED_BYTES_PER_SOCKET: usize = 4 * 1024 * 1024;

impl PacketIo {
    pub(crate) fn new(stream: tokio::net::TcpStream) -> Self {
        Self {
            stream,
            enc: PacketEncoder::new(),
            dec: PacketDecoder::new(),
            buf: BytesMut::new(),
        }
    }

    pub(crate) async fn send_packet<P>(&mut self, pkt: &P) -> anyhow::Result<()>
    where
        P: Packet + Encode,
    {
        self.enc.append_packet(pkt)?;
        let bytes = self.enc.take();
        self.stream.write_all(&bytes).await?;
        Ok(())
    }

    pub(crate) async fn recv_packet<'a, P>(&'a mut self) -> anyhow::Result<P>
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

    pub(crate) fn into_raw_connection(self, remote_addr: SocketAddr) -> RawConnection {
        let (incoming_sender, incoming_receiver) = mpsc::channel(256);
        let (outgoing_sender, outgoing_receiver) = mpsc::channel::<Bytes>(OUTBOUND_CHANNEL_CAPACITY);
        let disconnect_flag = Arc::new(AtomicBool::new(false));

        let (reader, writer) = self.stream.into_split();

        let reader_task = tokio::spawn(reader_loop(reader, self.dec, incoming_sender));
        let writer_task =
            tokio::spawn(writer_loop(outgoing_receiver, writer, disconnect_flag.clone()));

        RawConnection {
            outgoing: outgoing_sender,
            recv: incoming_receiver,
            reader_task,
            writer_task,
            enc: self.enc,
            remote_addr,
            disconnect_flag,
        }
    }
}

async fn reader_loop(
    mut reader: tokio::net::tcp::OwnedReadHalf,
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

async fn writer_loop(
    mut rx: mpsc::Receiver<Bytes>,
    tcp: tokio::net::tcp::OwnedWriteHalf,
    disconnect_flag: Arc<AtomicBool>,
) {
    let mut writer = BufWriter::with_capacity(64 * 1024, tcp);
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
    reader_task: JoinHandle<()>,
    // Held to keep the writer task alive; dropped implicitly when RawConnection is dropped.
    #[allow(dead_code)]
    writer_task: JoinHandle<()>,
    pub enc: PacketEncoder,
    pub remote_addr: SocketAddr,
    disconnect_flag: Arc<AtomicBool>,
}

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
    pub fn new_for_test_full(
        outgoing_capacity: usize,
    ) -> (
        Self,
        mpsc::Receiver<Bytes>,
        mpsc::Sender<ReceivedPacket>,
    ) {
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
