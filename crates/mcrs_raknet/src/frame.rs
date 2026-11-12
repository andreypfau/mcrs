extern crate alloc;

use bytes::Bytes;

use crate::reliability::Reliability;
use crate::wire::Uint24;

pub const MAX_UNSPLIT_PAYLOAD: usize = 8191;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SplitInfo {
    count: u32,
    id: u16,
    index: u32,
}

impl SplitInfo {
    #[must_use]
    pub const fn new(count: u32, id: u16, index: u32) -> Self {
        Self { count, id, index }
    }

    #[must_use]
    pub const fn count(self) -> u32 {
        self.count
    }

    #[must_use]
    pub const fn id(self) -> u16 {
        self.id
    }

    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub(crate) reliability: Reliability,
    pub(crate) reliable_index: Option<Uint24>,
    pub(crate) sequence_index: Option<Uint24>,
    pub(crate) order_index: Option<Uint24>,
    pub(crate) order_channel: u8,
    pub(crate) split_info: Option<SplitInfo>,
    pub(crate) payload: Bytes,
}

impl Frame {
    #[must_use]
    pub const fn new(reliability: Reliability, payload: Bytes) -> Self {
        Self {
            reliability,
            reliable_index: None,
            sequence_index: None,
            order_index: None,
            order_channel: 0,
            split_info: None,
            payload,
        }
    }

    #[must_use]
    pub const fn with_reliable_index(mut self, idx: Uint24) -> Self {
        self.reliable_index = Some(idx);
        self
    }

    #[must_use]
    pub const fn with_sequence_index(mut self, idx: Uint24) -> Self {
        self.sequence_index = Some(idx);
        self
    }

    #[must_use]
    pub const fn with_order(mut self, idx: Uint24, channel: u8) -> Self {
        self.order_index = Some(idx);
        self.order_channel = channel;
        self
    }

    #[must_use]
    pub const fn reliability(&self) -> Reliability {
        self.reliability
    }

    #[must_use]
    pub const fn reliable_index(&self) -> Option<Uint24> {
        self.reliable_index
    }

    #[must_use]
    pub const fn sequence_index(&self) -> Option<Uint24> {
        self.sequence_index
    }

    #[must_use]
    pub const fn order_index(&self) -> Option<Uint24> {
        self.order_index
    }

    #[must_use]
    pub const fn order_channel(&self) -> u8 {
        self.order_channel
    }

    #[must_use]
    pub const fn split_info(&self) -> Option<SplitInfo> {
        self.split_info
    }

    #[must_use]
    pub fn payload(&self) -> &Bytes {
        &self.payload
    }

    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<(Self, &[u8])> {
        let flags = *buf.first()?;
        let reliability = Reliability::from_u8(flags >> 5);
        let has_split = (flags & 0x10) != 0;

        let b1 = *buf.get(1)?;
        let b2 = *buf.get(2)?;
        let bit_len = u16::from_be_bytes([b1, b2]) as usize;
        let byte_len = bit_len.div_ceil(8);

        let mut r = buf.get(3..)?;

        let mut reliable_index = None;
        if reliability.is_reliable() {
            let (idx, rest) = Uint24::read_from(r)?;
            reliable_index = Some(idx);
            r = rest;
        }

        let mut sequence_index = None;
        if reliability.is_sequenced() {
            let (idx, rest) = Uint24::read_from(r)?;
            sequence_index = Some(idx);
            r = rest;
        }

        let mut order_index = None;
        let mut order_channel = 0;
        if reliability.is_ordered() || reliability.is_sequenced() {
            let (idx, rest) = Uint24::read_from(r)?;
            order_index = Some(idx);
            order_channel = *rest.first()?;
            r = rest.get(1..)?;
        }

        let mut split_info = None;
        if has_split {
            let count_bytes = r.get(..4)?;
            let count = u32::from_be_bytes([
                *count_bytes.first()?,
                *count_bytes.get(1)?,
                *count_bytes.get(2)?,
                *count_bytes.get(3)?,
            ]);
            let id_bytes = r.get(4..6)?;
            let id = u16::from_be_bytes([*id_bytes.first()?, *id_bytes.get(1)?]);
            let idx_bytes = r.get(6..10)?;
            let index = u32::from_be_bytes([
                *idx_bytes.first()?,
                *idx_bytes.get(1)?,
                *idx_bytes.get(2)?,
                *idx_bytes.get(3)?,
            ]);
            split_info = Some(SplitInfo { count, id, index });
            r = r.get(10..)?;
        }

        if r.len() < byte_len {
            return None;
        }
        let payload_slice = r.get(..byte_len)?;
        let payload = Bytes::copy_from_slice(payload_slice);
        let remaining = r.get(byte_len..)?;

        Some((
            Self {
                reliability,
                reliable_index,
                sequence_index,
                order_index,
                order_channel,
                split_info,
                payload,
            },
            remaining,
        ))
    }

    #[inline]
    #[must_use]
    pub fn wire_size(&self) -> usize {
        let mut size = 3;
        if self.reliability.is_reliable() {
            size += 3;
        }
        if self.reliability.is_sequenced() {
            size += 3;
        }
        if self.reliability.is_ordered() || self.reliability.is_sequenced() {
            size += 4;
        }
        if self.split_info.is_some() {
            size += 10;
        }
        size + self.payload.len()
    }

    pub fn write_to<'a>(&self, buf: &'a mut [u8]) -> Option<&'a mut [u8]> {
        if self.payload.len() > MAX_UNSPLIT_PAYLOAD && self.split_info.is_none() {
            return None;
        }
        let needed = self.wire_size();
        if buf.len() < needed {
            return None;
        }

        let mut flags = (self.reliability as u8) << 5;
        if self.split_info.is_some() {
            flags |= 0x10;
        }
        if let Some(slot) = buf.first_mut() {
            *slot = flags;
        }

        let bit_len = (self.payload.len() * 8) as u16;
        if let Some(dst) = buf.get_mut(1..3) {
            dst.copy_from_slice(&bit_len.to_be_bytes());
        }

        let mut offset = 3;

        if self.reliability.is_reliable() {
            let idx = self.reliable_index.unwrap_or(Uint24::new(0));
            let target = buf.get_mut(offset..offset + 3)?;
            idx.write_to(target)?;
            offset += 3;
        }

        if self.reliability.is_sequenced() {
            let idx = self.sequence_index.unwrap_or(Uint24::new(0));
            let target = buf.get_mut(offset..offset + 3)?;
            idx.write_to(target)?;
            offset += 3;
        }

        if self.reliability.is_ordered() || self.reliability.is_sequenced() {
            let idx = self.order_index.unwrap_or(Uint24::new(0));
            let target = buf.get_mut(offset..offset + 3)?;
            idx.write_to(target)?;
            offset += 3;
            if let Some(slot) = buf.get_mut(offset) {
                *slot = self.order_channel;
            }
            offset += 1;
        }

        if let Some(split) = self.split_info {
            let count_bytes = split.count.to_be_bytes();
            let id_bytes = split.id.to_be_bytes();
            let index_bytes = split.index.to_be_bytes();

            let target_count = buf.get_mut(offset..offset + 4)?;
            target_count.copy_from_slice(&count_bytes);
            offset += 4;

            let target_id = buf.get_mut(offset..offset + 2)?;
            target_id.copy_from_slice(&id_bytes);
            offset += 2;

            let target_index = buf.get_mut(offset..offset + 4)?;
            target_index.copy_from_slice(&index_bytes);
            offset += 4;
        }

        let payload_len = self.payload.len();
        let target_payload = buf.get_mut(offset..offset + payload_len)?;
        target_payload.copy_from_slice(self.payload.as_ref());
        offset += payload_len;

        buf.get_mut(offset..)
    }
}
