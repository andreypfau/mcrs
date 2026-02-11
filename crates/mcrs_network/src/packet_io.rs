use crate::{EngineConnection, ReceivedPacket};
use bytes::{Bytes, BytesMut};
use log::{error, warn};
use mcrs_protocol::{
    ConnectionState, Decode, Encode, Packet, PacketDecoder, PacketEncoder, WritePacket,
};
use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
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

    pub(crate) fn into_raw_connection(mut self, remote_addr: SocketAddr) -> RawConnection {
        let (incoming_sender, incoming_receiver) = mpsc::channel(256);
        let (outgoing_sender, outgoing_receiver) = mpsc::unbounded_channel::<Bytes>();
        let queued_bytes = Arc::new(AtomicUsize::new(0));

        let (reader, writer) = self.stream.into_split();

        let reader_task = tokio::spawn(reader_loop(reader, self.dec, incoming_sender));
        let writer_task =
            tokio::spawn(writer_loop(outgoing_receiver, writer, queued_bytes.clone()));

        RawConnection {
            outgoing: outgoing_sender,
            recv: incoming_receiver,
            reader_task,
            writer_task,
            enc: self.enc,
            remote_addr,
            queued_bytes,
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
    mut rx: mpsc::UnboundedReceiver<Bytes>,
    tcp: tokio::net::tcp::OwnedWriteHalf,
    queued_bytes: Arc<AtomicUsize>,
) {
    let mut writer = BufWriter::with_capacity(64 * 1024, tcp);
    while let Some(bytes) = rx.recv().await {
        let len = bytes.len();
        if writer.write_all(&bytes).await.is_err() {
            return;
        }
        // Decrement AFTER successful write so the counter accurately
        // reflects bytes not yet written to TCP.
        queued_bytes.fetch_sub(len, Ordering::Relaxed);

        // Drain all pending messages without awaiting the channel
        while let Ok(bytes) = rx.try_recv() {
            let len = bytes.len();
            if writer.write_all(&bytes).await.is_err() {
                return;
            }
            queued_bytes.fetch_sub(len, Ordering::Relaxed);
        }
        if writer.flush().await.is_err() {
            return;
        }
    }
    // Channel closed — drain any remaining buffered data for graceful shutdown.
    let _ = writer.flush().await;
}

pub(crate) struct RawConnection {
    outgoing: mpsc::UnboundedSender<Bytes>,
    recv: mpsc::Receiver<ReceivedPacket>,
    reader_task: JoinHandle<()>,
    writer_task: JoinHandle<()>,
    enc: PacketEncoder,
    pub remote_addr: SocketAddr,
    queued_bytes: Arc<AtomicUsize>,
}

impl Drop for RawConnection {
    fn drop(&mut self) {
        // Only abort the reader. The writer will drain remaining messages
        // and shut down naturally when the outgoing sender is dropped.
        self.reader_task.abort();
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
        let len = bytes.len();
        let bytes = bytes.freeze();
        // Increment BEFORE send so the writer task's fetch_sub never
        // underflows the counter (it can only subtract after receiving).
        self.queued_bytes.fetch_add(len, Ordering::Relaxed);
        self.outgoing.send(bytes).map_err(|_| {
            // Undo the increment since the bytes were not actually sent.
            self.queued_bytes.fetch_sub(len, Ordering::Relaxed);
            anyhow::anyhow!("connection closed")
        })?;
        Ok(())
    }

    fn queued_bytes(&self) -> usize {
        self.queued_bytes.load(Ordering::Relaxed)
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
