use anyhow::ensure;
use bytes::{BufMut, BytesMut};
use tracing::warn;

use crate::var_int::VarInt;
use crate::{CompressionThreshold, Encode, MAX_PACKET_SIZE, Packet};

#[derive(Default)]
pub struct PacketEncoder {
    buf: BytesMut,
    compress_buf: Vec<u8>,
    threshold: CompressionThreshold,
}

impl PacketEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append_packet<P>(&mut self, pkt: &P) -> anyhow::Result<()>
    where
        P: Packet + Encode,
    {
        let start_len = self.buf.len();

        pkt.encode_with_id((&mut self.buf).writer())?;

        let data_len = self.buf.len() - start_len;

        if self.threshold.0 >= 0 {
            use std::io::Read;

            use flate2::Compression;
            use flate2::bufread::ZlibEncoder;

            if data_len > self.threshold.0 as usize {
                let mut z = ZlibEncoder::new(&self.buf[start_len..], Compression::new(4));

                self.compress_buf.clear();

                let data_len_size = VarInt(data_len as i32).written_size();

                let packet_len = data_len_size + z.read_to_end(&mut self.compress_buf)?;

                ensure!(
                    packet_len <= MAX_PACKET_SIZE as usize,
                    "packet exceeds maximum length"
                );

                drop(z);

                self.buf.truncate(start_len);

                let mut writer = (&mut self.buf).writer();

                VarInt(packet_len as i32).encode(&mut writer)?;
                VarInt(data_len as i32).encode(&mut writer)?;
                self.buf.extend_from_slice(&self.compress_buf);
            } else {
                let data_len_size = 1;
                let packet_len = data_len_size + data_len;

                ensure!(
                    packet_len <= MAX_PACKET_SIZE as usize,
                    "packet exceeds maximum length"
                );

                let packet_len_size = VarInt(packet_len as i32).written_size();

                let data_prefix_len = packet_len_size + data_len_size;

                self.buf.put_bytes(0, data_prefix_len);
                self.buf
                    .copy_within(start_len..start_len + data_len, start_len + data_prefix_len);

                let mut front = &mut self.buf[start_len..];

                VarInt(packet_len as i32).encode(&mut front)?;
                // Zero for no compression on this packet.
                VarInt(0).encode(front)?;
            }

            return Ok(());
        }

        let packet_len = data_len;

        ensure!(
            packet_len <= MAX_PACKET_SIZE as usize,
            "packet exceeds maximum length"
        );

        let packet_len_size = VarInt(packet_len as i32).written_size();

        self.buf.put_bytes(0, packet_len_size);
        self.buf
            .copy_within(start_len..start_len + data_len, start_len + packet_len_size);

        let front = &mut self.buf[start_len..];
        VarInt(packet_len as i32).encode(front)?;

        Ok(())
    }

    pub fn take(&mut self) -> BytesMut {
        self.buf.split()
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }

    pub fn set_compression(&mut self, threshold: CompressionThreshold) {
        self.threshold = threshold;
    }
}

/// Types that can have packets written to them.
pub trait WritePacket {
    /// Writes a packet to this object. Encoding errors are typically logged and
    /// discarded.
    fn write_packet<P>(&mut self, packet: &P)
    where
        P: Packet + Encode,
    {
        if let Err(e) = self.write_packet_fallible(packet) {
            warn!("failed to write packet '{}': {e:#}", P::NAME);
        }
    }

    /// Writes a packet to this object. The result of encoding the packet is
    /// returned.
    fn write_packet_fallible<P>(&mut self, packet: &P) -> anyhow::Result<()>
    where
        P: Packet + Encode;
}

impl WritePacket for PacketEncoder {
    fn write_packet_fallible<P>(&mut self, packet: &P) -> anyhow::Result<()>
    where
        P: Packet + Encode,
    {
        self.append_packet(packet)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use bytes::Bytes;

    use super::*;
    use crate::frame::{FrameError, MAX_FRAME_BODY, MAX_UNCOMPRESSED_PACKET, split_frame};
    use crate::{ConnectionState, PacketDecoder, PacketSide};

    #[derive(Debug)]
    struct Raw(Vec<u8>);

    impl Packet for Raw {
        const ID: i32 = 0x2A;
        const NAME: &'static str = "Raw";
        const SIDE: PacketSide = PacketSide::Clientbound;
        const STATE: ConnectionState = ConnectionState::Game;
    }

    impl Encode for Raw {
        fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
            w.write_all(&self.0)?;
            Ok(())
        }
    }

    #[derive(Debug)]
    struct Failing;

    impl Packet for Failing {
        const ID: i32 = 0x2B;
        const NAME: &'static str = "Failing";
        const SIDE: PacketSide = PacketSide::Clientbound;
        const STATE: ConnectionState = ConnectionState::Game;
    }

    impl Encode for Failing {
        fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
            w.write_all(&[1, 2, 3])?;
            anyhow::bail!("refused after writing")
        }
    }

    fn packet_of(encoded_len: usize) -> Raw {
        Raw((0..encoded_len - 1).map(|i| (i * 7 % 251) as u8).collect())
    }

    fn encoder(threshold: i32) -> PacketEncoder {
        let mut encoder = PacketEncoder::new();
        encoder.set_compression(CompressionThreshold(threshold));
        encoder
    }

    fn data_length(encoder: &PacketEncoder) -> i32 {
        let mut buf = encoder.buf.clone();
        let frame = split_frame(&mut buf).unwrap();
        let mut rest = &frame[..];
        VarInt::decode_partial(&mut rest).unwrap()
    }

    fn read_all(encoder: &mut PacketEncoder, threshold: i32) -> Vec<(i32, Bytes)> {
        let mut decoder = PacketDecoder::new();
        decoder.set_compression(CompressionThreshold(threshold));
        decoder.queue_bytes(encoder.take());
        let mut frames = Vec::new();
        while let Some(frame) = decoder.try_next_packet().unwrap() {
            frames.push((frame.id, frame.body));
        }
        frames
    }

    fn expected(packet: &Raw) -> (i32, Bytes) {
        (Raw::ID, Bytes::copy_from_slice(&packet.0))
    }

    #[test]
    fn a_packet_of_exactly_the_threshold_is_compressed() {
        for (encoded_len, compressed) in [(255, false), (256, true), (257, true)] {
            let mut encoder = encoder(256);
            let packet = packet_of(encoded_len);
            encoder.append_packet(&packet).unwrap();
            assert_eq!(
                data_length(&encoder),
                if compressed { encoded_len as i32 } else { 0 },
                "{encoded_len} bytes"
            );
            assert_eq!(read_all(&mut encoder, 256), [expected(&packet)]);
        }
    }

    #[test]
    fn a_packet_that_fails_to_encode_leaves_nothing_in_the_buffer() {
        for threshold in [-1, 256] {
            let mut encoder = encoder(threshold);
            let first = packet_of(300);
            let last = packet_of(40);
            encoder.append_packet(&first).unwrap();
            let before = encoder.buf.clone();

            assert!(encoder.append_packet(&Failing).is_err());
            assert_eq!(encoder.buf, before, "threshold {threshold}");

            encoder.append_packet(&last).unwrap();
            assert_eq!(
                read_all(&mut encoder, threshold),
                [expected(&first), expected(&last)],
                "threshold {threshold}"
            );
        }
    }

    #[test]
    fn a_packet_the_frame_rules_refuse_leaves_nothing_in_the_buffer() {
        let cases = [
            (
                -1,
                MAX_FRAME_BODY + 1,
                FrameError::FrameTooLarge {
                    len: MAX_FRAME_BODY + 1,
                },
            ),
            (
                256,
                MAX_UNCOMPRESSED_PACKET + 1,
                FrameError::PacketTooLarge {
                    len: MAX_UNCOMPRESSED_PACKET + 1,
                },
            ),
        ];
        for (threshold, encoded_len, error) in cases {
            let mut encoder = encoder(threshold);
            let first = packet_of(300);
            let last = packet_of(40);
            encoder.append_packet(&first).unwrap();
            let before = encoder.buf.clone();

            let refused = encoder.append_packet(&packet_of(encoded_len)).unwrap_err();
            assert_eq!(
                refused.downcast_ref::<FrameError>(),
                Some(&error),
                "threshold {threshold}"
            );
            assert_eq!(encoder.buf, before, "threshold {threshold}");

            encoder.append_packet(&last).unwrap();
            assert_eq!(
                read_all(&mut encoder, threshold),
                [expected(&first), expected(&last)],
                "threshold {threshold}"
            );
        }
    }
}
