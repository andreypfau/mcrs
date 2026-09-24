use anyhow::{Context, bail, ensure};
use bytes::{Buf, BytesMut};

use crate::CompressionThreshold;
use crate::var_int::{VarInt, VarIntDecodeError};
use crate::{Decode, MAX_PACKET_SIZE};

#[derive(Default)]
pub struct PacketDecoder {
    buf: BytesMut,
    decompress_buf: BytesMut,
    threshold: CompressionThreshold,
}

impl PacketDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_next_packet(&mut self) -> anyhow::Result<Option<PacketFrame>> {
        let mut r = &self.buf[..];

        let packet_len = match VarInt::decode_partial(&mut r) {
            Ok(len) => len,
            Err(VarIntDecodeError::Incomplete) => return Ok(None),
            Err(VarIntDecodeError::TooLarge) => bail!("malformed packet length VarInt"),
        };

        ensure!(
            (0..=MAX_PACKET_SIZE).contains(&packet_len),
            "packet length of {packet_len} is out of bounds"
        );

        if r.len() < packet_len as usize {
            // Not enough data arrived yet.
            return Ok(None);
        }

        let packet_len_len = VarInt(packet_len).written_size();

        let mut data;

        if self.threshold.0 >= 0 {
            use std::io::Write;

            use bytes::BufMut;
            use flate2::write::ZlibDecoder;

            r = &r[..packet_len as usize];

            let data_len = VarInt::decode(&mut r)?.0;

            ensure!(
                (0..MAX_PACKET_SIZE).contains(&data_len),
                "decompressed packet length of {data_len} is out of bounds"
            );

            // Is this packet compressed?
            if data_len > 0 {
                ensure!(
                    data_len > self.threshold.0,
                    "decompressed packet length of {data_len} is <= the compression threshold of \
                     {}",
                    self.threshold.0
                );

                debug_assert!(self.decompress_buf.is_empty());

                self.decompress_buf.put_bytes(0, data_len as usize);

                // TODO: use libdeflater or zune-inflate?
                let mut z = ZlibDecoder::new(&mut self.decompress_buf[..]);

                z.write_all(r)?;

                ensure!(
                    z.finish()?.is_empty(),
                    "decompressed packet length is shorter than expected"
                );

                let total_packet_len = VarInt(packet_len).written_size() + packet_len as usize;

                self.buf.advance(total_packet_len);

                data = self.decompress_buf.split();
            } else {
                debug_assert_eq!(data_len, 0);

                ensure!(
                    r.len() <= self.threshold.0 as usize,
                    "uncompressed packet length of {} exceeds compression threshold of {}",
                    r.len(),
                    self.threshold.0
                );

                let remaining_len = r.len();

                self.buf.advance(packet_len_len + 1);

                data = self.buf.split_to(remaining_len);
            }
        } else {
            self.buf.advance(packet_len_len);
            data = self.buf.split_to(packet_len as usize);
        }

        // Decode the leading packet ID.
        r = &data[..];
        let packet_id = VarInt::decode(&mut r)
            .context("failed to decode packet ID")?
            .0;

        data.advance(data.len() - r.len());

        Ok(Some(PacketFrame {
            id: packet_id,
            body: data,
        }))
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
    pub body: BytesMut,
}
