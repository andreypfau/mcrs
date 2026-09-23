/// Fixed-width entries packed into `u64` words. Each word holds
/// `64 / bits_per_entry` entries and the lowest entry occupies the lowest bits,
/// with the spare high bits of each word left unused rather than split across a
/// word boundary.
#[derive(Debug, Clone)]
pub struct PackedBitStorage {
    longs: Vec<u64>,
    bits_per_entry: u8,
    entries_per_long: u8,
    entry_count: u32,
    max_value: u32,
}

impl PackedBitStorage {
    pub fn new(entry_count: usize, max_value: u32) -> Self {
        let bits_per_entry = bits_needed_for(max_value);
        Self::with_bits(entry_count, bits_per_entry, max_value)
    }

    pub fn with_bits(entry_count: usize, bits_per_entry: u8, max_value: u32) -> Self {
        debug_assert!(
            bits_per_entry > 0 && bits_per_entry <= 32,
            "bits_per_entry must be in 1..=32 (got {bits_per_entry})"
        );
        let entries_per_long = (64 / bits_per_entry as u32) as u8;
        let longs_needed = entry_count.div_ceil(entries_per_long as usize);
        Self {
            longs: vec![0u64; longs_needed],
            bits_per_entry,
            entries_per_long,
            entry_count: entry_count as u32,
            max_value,
        }
    }

    #[inline]
    pub fn get(&self, index: usize) -> u32 {
        debug_assert!(
            index < self.entry_count as usize,
            "PackedBitStorage::get index {index} out of range (entry_count={})",
            self.entry_count
        );
        let entries_per_long = self.entries_per_long as usize;
        let long_index = index / entries_per_long;
        let sub_index = index % entries_per_long;
        let shift = sub_index as u32 * self.bits_per_entry as u32;
        let mask: u64 = if self.bits_per_entry == 64 {
            u64::MAX
        } else {
            (1u64 << self.bits_per_entry) - 1
        };
        ((self.longs[long_index] >> shift) & mask) as u32
    }

    #[inline]
    pub fn set(&mut self, index: usize, value: u32) {
        debug_assert!(
            index < self.entry_count as usize,
            "PackedBitStorage::set index {index} out of range (entry_count={})",
            self.entry_count
        );
        debug_assert!(
            value <= self.max_value,
            "PackedBitStorage::set value {value} exceeds max_value {}",
            self.max_value
        );
        let entries_per_long = self.entries_per_long as usize;
        let long_index = index / entries_per_long;
        let sub_index = index % entries_per_long;
        let shift = sub_index as u32 * self.bits_per_entry as u32;
        let mask: u64 = if self.bits_per_entry == 64 {
            u64::MAX
        } else {
            (1u64 << self.bits_per_entry) - 1
        };
        let cleared = self.longs[long_index] & !(mask << shift);
        self.longs[long_index] = cleared | ((value as u64 & mask) << shift);
    }

    pub fn raw_longs(&self) -> &[u64] {
        &self.longs
    }

    pub fn bits_per_entry(&self) -> u8 {
        self.bits_per_entry
    }

    pub fn entry_count(&self) -> u32 {
        self.entry_count
    }
}

/// `ceil(log2(max_value + 1))` clamped to a minimum of 1.
pub fn bits_needed_for(max_value: u32) -> u8 {
    if max_value == 0 {
        return 1;
    }
    (32 - max_value.leading_zeros()) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pbs_8bit_set_get_round_trip() {
        let mut pbs = PackedBitStorage::new(256, 0xFF);
        assert_eq!(pbs.bits_per_entry(), 8);
        for i in 0..256 {
            pbs.set(i, (i as u32) & 0xFF);
        }
        for i in 0..256 {
            assert_eq!(pbs.get(i), (i as u32) & 0xFF, "mismatch at {i}");
        }
    }

    #[test]
    fn pbs_9bit_specific_byte_layout() {
        // 9 bits per entry; entries_per_long = 64 / 9 = 7.
        let mut pbs = PackedBitStorage::with_bits(256, 9, 0x1FF);
        pbs.set(0, 0x1FF);
        pbs.set(1, 0x000);
        pbs.set(2, 0x1FF);
        let expected = 0x1FFu64 | (0x1FFu64 << 18);
        assert_eq!(
            pbs.raw_longs()[0],
            expected,
            "lowest entry must occupy lowest bits of long"
        );
        assert_eq!(pbs.get(0), 0x1FF);
        assert_eq!(pbs.get(1), 0x000);
        assert_eq!(pbs.get(2), 0x1FF);
    }

    #[test]
    #[should_panic(expected = "exceeds max_value")]
    fn pbs_value_clipping_debug_assert() {
        let mut pbs = PackedBitStorage::new(64, 0xFF);
        pbs.set(0, 0x100);
    }

    #[test]
    fn pbs_long_count_matches_ceil() {
        // 256 entries at 9 bits/entry: 7 entries per long -> ceil(256/7) = 37 longs.
        let pbs = PackedBitStorage::with_bits(256, 9, 0x1FF);
        assert_eq!(pbs.raw_longs().len(), 37);
    }

    #[test]
    fn pbs_bits_needed_for_boundaries() {
        assert_eq!(bits_needed_for(0), 1);
        assert_eq!(bits_needed_for(1), 1);
        assert_eq!(bits_needed_for(2), 2);
        assert_eq!(bits_needed_for(255), 8);
        assert_eq!(bits_needed_for(256), 9);
        assert_eq!(bits_needed_for(384), 9);
        assert_eq!(bits_needed_for(511), 9);
        assert_eq!(bits_needed_for(512), 10);
    }
}
