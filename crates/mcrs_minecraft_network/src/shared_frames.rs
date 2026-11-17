use std::collections::HashMap;

use bytes::Bytes;
use mcrs_minecraft_protocol::{CompressionThreshold, PacketEncoder};

#[derive(Default)]
pub struct SharedFrames {
    packets: HashMap<PacketKey, SharedPacket>,
    encoder: PacketEncoder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedAppend {
    Unframed,
    FrameMade,
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

    pub fn clear(&mut self) {
        self.packets.clear();
    }

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
