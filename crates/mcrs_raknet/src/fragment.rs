extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use bytes::Bytes;

use crate::frame::SplitInfo;

pub const MAX_SPLIT_COUNT: u32 = 8192;
pub const MAX_ACTIVE_SPLITS: usize = 64;
pub const MAX_TOTAL_REASSEMBLY_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_SPLIT_TIMEOUT_MS: u64 = 10_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FragmentError {
    InvalidCount,
    InvalidIndex,
    TooManyActiveSplits,
    MemoryLimitExceeded,
    CountMismatch,
    CorruptedPart,
}

struct IncompleteSplit {
    count: u32,
    received_count: u32,
    total_len: usize,
    parts: BTreeMap<u32, Bytes>,
    created_at_ms: u64,
}

pub struct FragmentReassembler {
    active: BTreeMap<u16, IncompleteSplit>,
    total_buffered_bytes: usize,
    timeout_ms: u64,
}

impl Default for FragmentReassembler {
    fn default() -> Self {
        Self::new(DEFAULT_SPLIT_TIMEOUT_MS)
    }
}

impl FragmentReassembler {
    #[must_use]
    pub const fn new(timeout_ms: u64) -> Self {
        Self {
            active: BTreeMap::new(),
            total_buffered_bytes: 0,
            timeout_ms,
        }
    }

    #[must_use]
    pub fn active_splits_count(&self) -> usize {
        self.active.len()
    }

    #[must_use]
    pub fn total_buffered_bytes(&self) -> usize {
        self.total_buffered_bytes
    }

    pub fn insert(
        &mut self,
        split: SplitInfo,
        payload: Bytes,
        now_ms: u64,
    ) -> Result<Option<Bytes>, FragmentError> {
        if split.count() == 0 || split.count() > MAX_SPLIT_COUNT {
            return Err(FragmentError::InvalidCount);
        }
        if split.index() >= split.count() {
            return Err(FragmentError::InvalidIndex);
        }
        if split.count() == 1 {
            return Ok(Some(payload));
        }

        let payload_len = payload.len();
        if self.total_buffered_bytes.saturating_add(payload_len) > MAX_TOTAL_REASSEMBLY_BYTES {
            return Err(FragmentError::MemoryLimitExceeded);
        }

        if !self.active.contains_key(&split.id()) && self.active.len() >= MAX_ACTIVE_SPLITS {
            return Err(FragmentError::TooManyActiveSplits);
        }

        let id = split.id();
        let count = split.count();
        let index = split.index();

        let entry = self.active.entry(id).or_insert_with(|| IncompleteSplit {
            count,
            received_count: 0,
            total_len: 0,
            parts: BTreeMap::new(),
            created_at_ms: now_ms,
        });

        if entry.count != count {
            return Err(FragmentError::CountMismatch);
        }

        if let alloc::collections::btree_map::Entry::Vacant(vacant) = entry.parts.entry(index) {
            vacant.insert(payload);
            entry.received_count += 1;
            entry.total_len += payload_len;
            self.total_buffered_bytes += payload_len;
        }

        if entry.received_count == entry.count {
            let split_data = self
                .active
                .remove(&id)
                .ok_or(FragmentError::CorruptedPart)?;
            self.total_buffered_bytes = self
                .total_buffered_bytes
                .saturating_sub(split_data.total_len);

            let mut assembled = Vec::with_capacity(split_data.total_len);
            for part_idx in 0..split_data.count {
                let bytes = split_data
                    .parts
                    .get(&part_idx)
                    .ok_or(FragmentError::CorruptedPart)?;
                assembled.extend_from_slice(bytes.as_ref());
            }
            Ok(Some(Bytes::from(assembled)))
        } else {
            Ok(None)
        }
    }

    pub fn prune(&mut self, now_ms: u64) {
        let timeout = self.timeout_ms;
        let mut expired_bytes = 0usize;
        self.active.retain(|_, split| {
            let is_alive = now_ms.saturating_sub(split.created_at_ms) <= timeout;
            if !is_alive {
                expired_bytes += split.total_len;
            }
            is_alive
        });
        self.total_buffered_bytes = self.total_buffered_bytes.saturating_sub(expired_bytes);
    }

    #[must_use]
    pub fn split_payload(payload: &[u8], split_id: u16, max_part_size: usize) -> Vec<SplitInfo> {
        if max_part_size == 0 || payload.is_empty() {
            return Vec::new();
        }
        let count = payload.len().div_ceil(max_part_size) as u32;
        if count > MAX_SPLIT_COUNT {
            return Vec::new();
        }
        let mut splits = Vec::with_capacity(count as usize);
        for index in 0..count {
            splits.push(SplitInfo::new(count, split_id, index));
        }
        splits
    }
}
