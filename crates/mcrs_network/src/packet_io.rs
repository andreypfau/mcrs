use crate::byte_channel::{ByteSender, TrySendError, byte_channel};
use crate::{EngineConnection, ReceivedPacket};
use bytes::BytesMut;
use log::{error, warn};
use mcrs_protocol::{
    ConnectionState, Decode, Encode, Packet, PacketDecoder, PacketEncoder, WritePacket,
};
use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{Receiver, channel};
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

    pub(crate) fn into_raw_connection(
        mut self,
        remote_addr: SocketAddr,
        state: ConnectionState,
    ) -> RawConnection {
        let (incoming_sender, incoming_receiver) = channel(1);
        let (mut reader, mut writer) = self.stream.into_split();
        let reader_task = tokio::spawn(async move {
            let mut buf = BytesMut::new();
            loop {
                let frame = match self.dec.try_next_packet() {
                    Ok(Some(frame)) => frame,
                    Ok(None) => {
                        buf.reserve(READ_BUF_SIZE);
                        match reader.read_buf(&mut buf).await {
                            Ok(0) => break, // Reader is at EOF.
                            Ok(_) => {}
                            Err(e) => {
                                error!("error reading data from stream: {e}");
                                break;
                            }
                        }
                        self.dec.queue_bytes(buf.split());
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
                    break;
                }
            }
        });

        let (outgoing_sender, mut outgoing_receiver) = byte_channel(8388608);
        let writer_task = tokio::spawn(async move {
            loop {
                let bytes = match outgoing_receiver.recv_async().await {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        break;
                    }
                };
                // println!("writing packet of size {}", bytes.len());

                if let Err(e) = writer.write_all(&bytes).await {
                    // eprintln!("error writing data to stream: {e}");
                }
            }
        });

        RawConnection {
            send: outgoing_sender,
            recv: incoming_receiver,
            reader_task,
            writer_task,
            enc: self.enc,
            remote_addr,
            state,
        }
    }
}

pub(crate) struct RawConnection {
    send: ByteSender,
    recv: Receiver<ReceivedPacket>,
    reader_task: JoinHandle<()>,
    writer_task: JoinHandle<()>,
    enc: PacketEncoder,
    pub remote_addr: SocketAddr,
    pub state: ConnectionState,
}

impl EngineConnection for RawConnection {
    fn try_send(&mut self, bytes: BytesMut) -> Result<(), TrySendError> {
        self.send.try_send(bytes)
    }

    fn try_recv(&mut self) -> Result<Option<ReceivedPacket>, TryRecvError> {
        match self.recv.try_recv() {
            Ok(packet) => Ok(Some(packet)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(TryRecvError::Disconnected),
        }
    }

    fn flush(&mut self) -> Result<(), TrySendError> {
        let bytes = self.enc.take();
        if bytes.is_empty() {
            return Ok(());
        }
        self.send.try_send(bytes)
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
