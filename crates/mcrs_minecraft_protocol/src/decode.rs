use anyhow::Context;
use bytes::{Bytes, BytesMut};
use flate2::Decompress;

use crate::CompressionThreshold;
use crate::Decode;
use crate::frame::{decompress, split_frame};
use crate::var_int::VarInt;

#[derive(Default)]
pub struct PacketDecoder {
    buf: BytesMut,
    inflater: Option<Decompress>,
    threshold: CompressionThreshold,
}

impl PacketDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_next_packet(&mut self) -> anyhow::Result<Option<PacketFrame>> {
        let frame = match split_frame(&mut self.buf) {
            Ok(frame) => frame,
            Err(error) if error.is_incomplete() => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let packet = decompress(frame, self.threshold, &mut self.inflater)?;

        let mut rest = &packet[..];
        let id = VarInt::decode(&mut rest)
            .context("failed to decode packet ID")?
            .0;
        let body = packet.slice(packet.len() - rest.len()..);

        Ok(Some(PacketFrame { id, body }))
    }

    pub fn set_compression(&mut self, threshold: CompressionThreshold) {
        self.threshold = threshold;
    }

    pub fn queue_bytes(&mut self, bytes: BytesMut) {
        self.buf.unsplit(bytes);
    }

    pub fn take_capacity(&mut self) -> BytesMut {
        self.buf.split_off(self.buf.len())
    }

    pub fn reserve(&mut self, additional: usize) {
        self.buf.reserve(additional);
    }
}

#[derive(Clone, Debug)]
pub struct PacketFrame {
    /// The ID of the decoded packet.
    pub id: i32,
    /// The contents of the packet after the leading VarInt ID.
    pub body: Bytes,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::encode_frame;

    const PACKET_ID: u8 = 0x2A;

    fn packet(len: usize) -> Vec<u8> {
        let mut packet: Vec<u8> = (0..len).map(|i| (i * 7 % 251) as u8).collect();
        packet[0] = PACKET_ID;
        packet
    }

    fn frame_of(packet: &[u8], threshold: i32) -> BytesMut {
        let mut buf = BytesMut::from(packet);
        encode_frame(&mut buf, 0, CompressionThreshold(threshold), &mut None).unwrap();
        buf
    }

    fn decoder(threshold: i32, input: &[u8]) -> PacketDecoder {
        let mut decoder = PacketDecoder::new();
        decoder.set_compression(CompressionThreshold(threshold));
        decoder.queue_bytes(BytesMut::from(input));
        decoder
    }

    fn assert_decodes(decoder: &mut PacketDecoder, packet: &[u8]) {
        let frame = decoder.try_next_packet().unwrap().unwrap();
        assert_eq!(frame.id, i32::from(PACKET_ID));
        assert_eq!(&frame.body[..], &packet[1..]);
    }

    #[test]
    fn the_decoder_waits_for_more_bytes_on_an_incomplete_frame() {
        let packet = packet(40);
        let frame = frame_of(&packet, -1);
        let mut decoder = PacketDecoder::new();

        assert!(decoder.try_next_packet().unwrap().is_none());
        decoder.queue_bytes(BytesMut::from(&frame[..1]));
        assert!(decoder.try_next_packet().unwrap().is_none());
        decoder.queue_bytes(BytesMut::from(&frame[1..frame.len() / 2]));
        assert!(decoder.try_next_packet().unwrap().is_none());
        decoder.queue_bytes(BytesMut::from(&frame[frame.len() / 2..]));
        assert_decodes(&mut decoder, &packet);
        assert!(decoder.try_next_packet().unwrap().is_none());

        for input in [&[0x80, 0x80, 0x80, 0x01][..], &[0x00][..]] {
            for threshold in [-1, 256] {
                let mut decoder = self::decoder(threshold, input);
                assert!(
                    decoder.try_next_packet().is_err(),
                    "{input:?} at threshold {threshold}"
                );
            }
        }
    }

    #[test]
    fn an_empty_packet_is_refused() {
        let mut decoder = decoder(256, &[0x01, 0x00]);
        assert!(decoder.try_next_packet().is_err());
    }

    #[test]
    fn a_threshold_set_between_two_calls_applies_to_buffered_frames() {
        let first = packet(300);
        let second = packet(400);
        let mut input = frame_of(&first, -1);
        input.extend_from_slice(&frame_of(&second, 256));
        let mut decoder = decoder(-1, &input);

        assert_decodes(&mut decoder, &first);
        decoder.set_compression(CompressionThreshold(256));
        assert_decodes(&mut decoder, &second);
        assert!(decoder.try_next_packet().unwrap().is_none());
    }
}
