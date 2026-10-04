use bytes::{BufMut, BytesMut};
use tracing::warn;

use crate::frame::{Deflate, encode_frame};
use crate::{CompressionThreshold, Encode, Packet};

#[derive(Default)]
pub struct PacketEncoder {
    buf: BytesMut,
    deflate: Option<Deflate>,
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
        let start = self.buf.len();

        if let Err(error) = pkt.encode_with_id((&mut self.buf).writer()) {
            self.buf.truncate(start);
            return Err(error);
        }

        encode_frame(&mut self.buf, start, self.threshold, &mut self.deflate)?;
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
            warn!(
                "failed to write {:?} {:?} packet '{}': {e:#}",
                P::STATE,
                P::SIDE,
                P::NAME
            );
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
    use crate::frame::{FrameError, MAX_FRAME_BODY, MAX_UNCOMPRESSED_PACKET};
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
    fn a_packet_that_fails_to_encode_leaves_nothing_in_the_buffer() {
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

            assert!(encoder.append_packet(&Failing).is_err());
            assert_eq!(encoder.buf, before, "threshold {threshold}");

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
