extern crate alloc;

use alloc::vec::Vec;

use crate::frame::Frame;
use crate::unconnected::id;
use crate::wire::Uint24;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Datagram {
    pub(crate) flags: u8,
    pub(crate) seq_number: Uint24,
    pub(crate) frames: Vec<Frame>,
}

impl Datagram {
    #[must_use]
    pub const fn new(seq_number: Uint24) -> Self {
        Self {
            flags: id::FRAME_SET_MIN,
            seq_number,
            frames: Vec::new(),
        }
    }

    #[must_use]
    pub const fn seq_number(&self) -> Uint24 {
        self.seq_number
    }

    #[must_use]
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    pub fn push_frame(&mut self, frame: Frame) {
        self.frames.push(frame);
    }

    #[must_use]
    pub fn contains_reliable_frame(&self) -> bool {
        self.frames.iter().any(|f| f.reliability.is_reliable())
    }

    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        let flags = *buf.first()?;
        if !(id::FRAME_SET_MIN..=id::FRAME_SET_MAX).contains(&flags) {
            return None;
        }

        let r = buf.get(1..)?;
        let (seq_number, mut rest) = Uint24::read_from(r)?;

        let mut frames = Vec::new();
        while !rest.is_empty() {
            let (frame, remaining) = Frame::read_from(rest)?;
            frames.push(frame);
            rest = remaining;
        }

        Some(Self {
            flags,
            seq_number,
            frames,
        })
    }

    #[inline]
    #[must_use]
    pub fn wire_size(&self) -> usize {
        let mut size = 4;
        for frame in &self.frames {
            size += frame.wire_size();
        }
        size
    }

    pub fn write_to(&self, buf: &mut [u8]) -> Option<usize> {
        let needed = self.wire_size();
        if buf.len() < needed {
            return None;
        }

        if let Some(slot) = buf.first_mut() {
            *slot = self.flags;
        }

        let target_seq = buf.get_mut(1..4)?;
        self.seq_number.write_to(target_seq)?;

        let total_len = buf.len();
        let mut offset = 4;
        for frame in &self.frames {
            let target = buf.get_mut(offset..)?;
            let rest = frame.write_to(target)?;
            offset = total_len - rest.len();
        }

        Some(offset)
    }
}
