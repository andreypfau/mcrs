use std::collections::HashMap;

use bytes::Bytes;
use mcrs_minecraft_protocol::{CompressionThreshold, PacketEncoder};

/// The packets that more than one connection receives in one flush, each serialized once by
/// `encode_with_id`, with one frame for each compression threshold in use.
///
/// A packet is known by the address and the length of its bytes. The entry holds the bytes, so
/// no other packet can take that address before [`SharedFrames::clear`].
#[derive(Default)]
pub struct SharedFrames {
    packets: HashMap<PacketKey, SharedPacket>,
    encoder: PacketEncoder,
}

/// How [`crate::RawConnection::append_shared`] queued a packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedAppend {
    /// The writer frames the packet: no other connection receives it in this flush.
    Unframed,
    /// The frame for this threshold was made here, which compresses the packet once at most.
    FrameMade,
    /// The frame made for another connection at this threshold went as it is.
    FrameReused,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PacketKey {
    address: usize,
    len: usize,
}

impl PacketKey {
    fn of(packet: &Bytes) -> Self {
        Self {
            address: packet.as_ptr().addr(),
            len: packet.len(),
        }
    }
}

struct SharedPacket {
    _packet: Bytes,
    recipients: u32,
    frames: Vec<ThresholdFrame>,
}

struct ThresholdFrame {
    compression: CompressionThreshold,
    frame: Bytes,
}

pub(crate) struct FoundFrame {
    pub(crate) frame: Bytes,
    pub(crate) append: SharedAppend,
}

impl SharedFrames {
    /// Counts one more connection that receives `packet` before the next [`Self::clear`].
    pub fn add_recipient(&mut self, packet: &Bytes) {
        let entry = self
            .packets
            .entry(PacketKey::of(packet))
            .or_insert_with(|| SharedPacket {
                _packet: packet.clone(),
                recipients: 0,
                frames: Vec::new(),
            });
        entry.recipients = entry.recipients.saturating_add(1);
    }

    /// Forgets every packet and its frames.
    pub fn clear(&mut self) {
        self.packets.clear();
    }

    /// The frame of `packet` at `compression`, made on the first call for that threshold.
    /// `None` when fewer than two connections receive the packet: its writer frames it then.
    pub(crate) fn frame(
        &mut self,
        packet: &Bytes,
        compression: CompressionThreshold,
    ) -> anyhow::Result<Option<FoundFrame>> {
        let Some(entry) = self.packets.get_mut(&PacketKey::of(packet)) else {
            return Ok(None);
        };
        if entry.recipients < 2 {
            return Ok(None);
        }
        if let Some(made) = entry
            .frames
            .iter()
            .find(|made| made.compression == compression)
        {
            return Ok(Some(FoundFrame {
                frame: made.frame.clone(),
                append: SharedAppend::FrameReused,
            }));
        }
        self.encoder.clear();
        self.encoder.set_compression(compression);
        self.encoder.append_encoded(packet)?;
        let frame = self.encoder.take().freeze();
        entry.frames.push(ThresholdFrame {
            compression,
            frame: frame.clone(),
        });
        Ok(Some(FoundFrame {
            frame,
            append: SharedAppend::FrameMade,
        }))
    }
}
