use std::io::{ErrorKind, Read};
use async_compression::tokio::bufread::ZlibDecoder;
use bytes::{Bytes, BytesMut};
use cipher::BlockSizeUser;
use mcrs_protocol::{CompressionThreshold, VarInt};
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, BufReader, ReadBuf};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender};
use mcrs_protocol::decode::PacketFrame;
use mcrs_protocol::var_int::VarIntDecodeError;

struct NetworkPlugin;

struct JavaClient {
    address: SocketAddr,
    outgoing_packet_queue_send: Sender<Bytes>,
    outgoing_packet_queue_recv: Option<Receiver<Bytes>>,
    reader: TCPNetworkDecoder<BufReader<OwnedReadHalf>>
}

impl JavaClient {
    pub fn new(tcp_stream: TcpStream, address: SocketAddr) -> Self {
        let (read, write) = tcp_stream.into_split();
        let (send, recv) = tokio::sync::mpsc::channel(128);
        Self {
            address,
            outgoing_packet_queue_send: send,
            outgoing_packet_queue_recv: Some(recv),
            reader: TCPNetworkDecoder::new(BufReader::new(read)),
        }
    }
}

// decrypt -> decompress -> raw
pub enum DecompressionReader<R: AsyncRead + Unpin> {
    Decompress(ZlibDecoder<BufReader<R>>),
    None(R),
}

impl<R: AsyncRead + Unpin> AsyncRead for DecompressionReader<R> {
    #[inline]
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Decompress(reader) => {
                let reader = std::pin::Pin::new(reader);
                reader.poll_read(cx, buf)
            }
            Self::None(reader) => {
                let reader = Pin::new(reader);
                reader.poll_read(cx, buf)
            }
        }
    }
}

pub enum DecryptionReader<R: AsyncRead + Unpin> {
    // Decrypt(Box<StreamDecryptor<R>>),
    None(R),
}

impl<R: AsyncRead + Unpin> AsyncRead for DecryptionReader<R> {
    #[inline]
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            // Self::Decrypt(reader) => {
            //     let reader = std::pin::Pin::new(reader);
            //     reader.poll_read(cx, buf)
            // }
            Self::None(reader) => {
                let reader = std::pin::Pin::new(reader);
                reader.poll_read(cx, buf)
            }
        }
    }
}

/// Decoder: Client -> Server
/// Supports ZLib decoding/decompression
/// Supports Aes128 Encryption
pub struct TCPNetworkDecoder<R: AsyncRead + Unpin> {
    reader: DecryptionReader<R>,
    compression: Option<CompressionThreshold>,
}

impl<R: AsyncRead + Unpin> TCPNetworkDecoder<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: DecryptionReader::None(reader),
            compression: None,
        }
    }

    pub fn set_compression(&mut self, threshold: CompressionThreshold) {
        self.compression = Some(threshold);
    }

    pub async fn read_raw_packet(&mut self) -> Result<PacketFrame, ()> {
        let Some(packet_len) = decode_partial_async(&mut self.reader).await else {
            return Err(());
        };
        let bounded_reader = (&mut self.reader).take(packet_len.0 as u64);
        let mut reader = DecompressionReader::None(bounded_reader);

        let Some(packet_id) = decode_partial_async(&mut reader)
            .await else {
            return Err(());
        };

        let mut payload = Vec::new();
        reader
            .read_to_end(&mut payload)
            .await
            .map_err(|_| ())?;

        Ok(PacketFrame {
            id: packet_id.0,
            body: payload[..].into(),
        })
    }
}


pub async fn decode_partial_async(
    reader: &mut (impl AsyncRead + Unpin),
) -> Option<VarInt> {
    let mut val = 0;
    for i in 0..5 {
        let byte = reader.read_u8().await;
        match byte {
            Ok(byte) => {
                val |= (i32::from(byte) & 0x7F) << (i * 7);
                if byte & 0x80 == 0 {
                    return Some(VarInt(val))
                }
            }
            Err(_) => {
                return None
            }
        }
    }
    None
}

pub enum EncryptionWriter<W: AsyncWrite + Unpin> {
    // Encrypt(Box<StreamEncryptor<W>>),
    None(W),
}

/// Encoder: Server -> Client
/// Supports ZLib endecoding/compression
/// Supports Aes128 Encryption
pub struct TCPNetworkEncoder<W: AsyncWrite + Unpin> {
    writer: EncryptionWriter<W>,
    // compression and compression threshold
    compression: Option<(CompressionThreshold, u32)>,
}
