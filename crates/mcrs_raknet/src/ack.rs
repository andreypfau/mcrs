extern crate alloc;

use alloc::vec::Vec;

use crate::unconnected::id;
use crate::wire::Uint24;

pub const MAX_RECORDS_PER_ACK: usize = 256;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AckRecord {
    Single(Uint24),
    Range(Uint24, Uint24),
}

impl AckRecord {
    #[must_use]
    pub fn contains(self, seq: Uint24) -> bool {
        match self {
            Self::Single(s) => s == seq,
            Self::Range(start, end) => Uint24::is_in_range_wrapped(seq, start, end),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AckPacket {
    is_nack: bool,
    records: Vec<AckRecord>,
}

impl AckPacket {
    #[must_use]
    pub const fn new(is_nack: bool, records: Vec<AckRecord>) -> Self {
        Self { is_nack, records }
    }

    #[must_use]
    pub const fn is_nack(&self) -> bool {
        self.is_nack
    }

    #[must_use]
    pub fn records(&self) -> &[AckRecord] {
        &self.records
    }

    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        let header = *buf.first()?;
        let is_nack = match header {
            id::ACK => false,
            id::NACK => true,
            _ => return None,
        };

        let b1 = *buf.get(1)?;
        let b2 = *buf.get(2)?;
        let record_count = u16::from_be_bytes([b1, b2]) as usize;
        let mut r = buf.get(3..)?;
        let mut records = Vec::with_capacity(record_count.min(MAX_RECORDS_PER_ACK));

        for _ in 0..record_count {
            if r.is_empty() {
                return None;
            }
            let is_single = *r.first()? != 0;
            r = r.get(1..)?;
            if is_single {
                let (seq, rest) = Uint24::read_from(r)?;
                records.push(AckRecord::Single(seq));
                r = rest;
            } else {
                let (start, rest) = Uint24::read_from(r)?;
                let (end, rest2) = Uint24::read_from(rest)?;
                records.push(AckRecord::Range(start, end));
                r = rest2;
            }
        }

        Some(Self { is_nack, records })
    }

    #[inline]
    #[must_use]
    pub fn wire_size(&self) -> usize {
        let mut size = 3;
        for rec in &self.records {
            match rec {
                AckRecord::Single(_) => size += 4,
                AckRecord::Range(_, _) => size += 7,
            }
        }
        size
    }

    pub fn write_to(&self, buf: &mut [u8]) -> Option<usize> {
        let needed = self.wire_size();
        if buf.len() < needed || self.records.len() > u16::MAX as usize {
            return None;
        }

        if let Some(slot) = buf.first_mut() {
            *slot = if self.is_nack { id::NACK } else { id::ACK };
        }
        let record_count = self.records.len() as u16;
        if let Some(dst) = buf.get_mut(1..3) {
            dst.copy_from_slice(&record_count.to_be_bytes());
        }

        let mut offset = 3;
        for rec in &self.records {
            match *rec {
                AckRecord::Single(seq) => {
                    if let Some(slot) = buf.get_mut(offset) {
                        *slot = 1;
                    }
                    offset += 1;
                    let target = buf.get_mut(offset..offset + 3)?;
                    seq.write_to(target)?;
                    offset += 3;
                }
                AckRecord::Range(start, end) => {
                    if let Some(slot) = buf.get_mut(offset) {
                        *slot = 0;
                    }
                    offset += 1;
                    let target_start = buf.get_mut(offset..offset + 3)?;
                    start.write_to(target_start)?;
                    offset += 3;
                    let target_end = buf.get_mut(offset..offset + 3)?;
                    end.write_to(target_end)?;
                    offset += 3;
                }
            }
        }

        Some(offset)
    }

    #[must_use]
    pub fn from_sequences(sequences: &[Uint24], is_nack: bool) -> Self {
        if sequences.is_empty() {
            return Self {
                is_nack,
                records: Vec::new(),
            };
        }

        let mut sorted = sequences.to_vec();
        sorted.sort_unstable_by_key(|a| a.as_u32());

        let mut records = Vec::new();
        let Some(&first) = sorted.first() else {
            return Self {
                is_nack,
                records: Vec::new(),
            };
        };

        let mut start = first;
        let mut end = first;

        for &seq in sorted.iter().skip(1) {
            if records.len() >= MAX_RECORDS_PER_ACK {
                break;
            }
            if seq.as_u32() == end.as_u32().wrapping_add(1) & 0x00FF_FFFF {
                end = seq;
            } else if seq == end {
                continue;
            } else {
                if start == end {
                    records.push(AckRecord::Single(start));
                } else {
                    records.push(AckRecord::Range(start, end));
                }
                start = seq;
                end = seq;
            }
        }

        if records.len() < MAX_RECORDS_PER_ACK {
            if start == end {
                records.push(AckRecord::Single(start));
            } else {
                records.push(AckRecord::Range(start, end));
            }
        }

        Self { is_nack, records }
    }
}
