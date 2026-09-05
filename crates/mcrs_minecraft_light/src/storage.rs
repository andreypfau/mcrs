use std::sync::Arc;

use mcrs_voxel_math::chunk_pos::BLOCKS;

use mcrs_voxel_storage::SectionNibbles;

/// `Eq` is load-bearing: `Arc` compares its pointers first only when the payload
/// is `Eq`, and that shortcut is why the dense payload is shared rather than
/// owned.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LightStorage {
    #[default]
    Empty,
    Uniform(u8),
    Dense(Arc<SectionNibbles>),
}

impl LightStorage {
    /// One byte per cell, in [`SectionNibbles::index`] order — the layout the
    /// working field uses. Storage stays nibble-packed; the hot loop never does.
    pub fn from_field(cells: &[u8; BLOCKS::VOLUME]) -> Self {
        // Most sections of a working field come back dark or fully lit, and
        // packing 2048 bytes only to throw them away is the whole cost of
        // reading a section back.
        let first = cells[0] & 0x0F;
        if cells.iter().all(|&c| c & 0x0F == first) {
            return match first {
                0 => LightStorage::Empty,
                value => LightStorage::Uniform(value),
            };
        }
        let mut arr = SectionNibbles::zeros();
        for (byte, pair) in arr.0.iter_mut().zip(cells.chunks_exact(2)) {
            *byte = (pair[0] & 0x0F) | ((pair[1] & 0x0F) << 4);
        }
        LightStorage::Dense(Arc::new(arr))
    }

    pub fn write_field(&self, cells: &mut [u8; BLOCKS::VOLUME]) {
        match self {
            LightStorage::Empty => cells.fill(0),
            LightStorage::Uniform(v) => cells.fill(*v),
            LightStorage::Dense(arr) => {
                for (byte, pair) in arr.0.iter().zip(cells.chunks_exact_mut(2)) {
                    pair[0] = byte & 0x0F;
                    pair[1] = byte >> 4;
                }
            }
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u8 {
        match self {
            LightStorage::Empty => 0,
            LightStorage::Uniform(v) => *v,
            LightStorage::Dense(arr) => arr.get(x, y, z),
        }
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, val: u8) {
        debug_assert!(val < 16);
        match self {
            LightStorage::Empty => {
                if val != 0 {
                    let mut arr = SectionNibbles::zeros();
                    arr.set(x, y, z, val);
                    *self = LightStorage::Dense(Arc::new(arr));
                }
            }
            LightStorage::Uniform(current) => {
                if *current == val {
                    return;
                }
                let mut arr = SectionNibbles::filled(*current);
                arr.set(x, y, z, val);
                *self = LightStorage::Dense(Arc::new(arr));
            }
            LightStorage::Dense(arr) => {
                Arc::make_mut(arr).set(x, y, z, val);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field_of(f: impl Fn(usize) -> u8) -> Box<[u8; BLOCKS::VOLUME]> {
        let mut cells = Box::new([0u8; BLOCKS::VOLUME]);
        for (i, cell) in cells.iter_mut().enumerate() {
            *cell = f(i);
        }
        cells
    }

    #[test]
    fn an_all_zero_field_normalizes_to_empty() {
        let cells = field_of(|_| 0);
        assert!(matches!(
            LightStorage::from_field(&cells),
            LightStorage::Empty
        ));
    }

    #[test]
    fn a_constant_field_normalizes_to_uniform() {
        let cells = field_of(|_| 12);
        assert!(matches!(
            LightStorage::from_field(&cells),
            LightStorage::Uniform(12)
        ));
    }

    #[test]
    fn field_round_trips_through_storage() {
        let cells = field_of(|i| (i % 16) as u8);
        let storage = LightStorage::from_field(&cells);
        assert!(matches!(storage, LightStorage::Dense(_)));
        assert_eq!(storage.get(3, 7, 11), cells[SectionNibbles::index(3, 7, 11)]);

        let mut back = Box::new([0u8; BLOCKS::VOLUME]);
        storage.write_field(&mut back);
        assert_eq!(&back[..], &cells[..]);
    }

    #[test]
    fn empty_and_uniform_expand_to_constant_fields() {
        let mut cells = Box::new([9u8; BLOCKS::VOLUME]);
        LightStorage::Empty.write_field(&mut cells);
        assert!(cells.iter().all(|&c| c == 0));

        LightStorage::Uniform(4).write_field(&mut cells);
        assert!(cells.iter().all(|&c| c == 4));
        assert!(matches!(
            LightStorage::from_field(&cells),
            LightStorage::Uniform(4)
        ));
    }

    #[test]
    fn default_is_null() {
        let s = LightStorage::default();
        assert!(matches!(s, LightStorage::Empty));
    }

    #[test]
    fn null_set_zero_stays_null() {
        let mut s = LightStorage::Empty;
        s.set(0, 0, 0, 0);
        assert!(matches!(s, LightStorage::Empty));
        s.set(7, 3, 9, 0);
        assert!(matches!(s, LightStorage::Empty));
    }

    #[test]
    fn null_set_nonzero_becomes_mixed_with_single_cell() {
        let mut s = LightStorage::Empty;
        s.set(3, 7, 11, 9);
        assert!(matches!(s, LightStorage::Dense(_)));
        assert_eq!(s.get(3, 7, 11), 9, "written cell holds the value");
        assert_eq!(s.get(0, 0, 0), 0, "other cells remain zero");
        assert_eq!(s.get(15, 15, 15), 0, "other cells remain zero");
    }

    #[test]
    fn uniform_set_same_stays_uniform() {
        let mut s = LightStorage::Uniform(5);
        s.set(0, 0, 0, 5);
        assert!(matches!(s, LightStorage::Uniform(5)));
        s.set(10, 10, 10, 5);
        assert!(matches!(s, LightStorage::Uniform(5)));
    }

    #[test]
    fn uniform_set_different_becomes_mixed_correctly() {
        let mut s = LightStorage::Uniform(5);
        s.set(0, 0, 0, 9);
        assert!(matches!(s, LightStorage::Dense(_)));
        assert_eq!(s.get(0, 0, 0), 9);
        assert_eq!(s.get(1, 2, 3), 5);
        assert_eq!(s.get(15, 15, 15), 5);
    }

    #[test]
    fn mixed_set_writes_through() {
        let mut s = LightStorage::Uniform(5);
        s.set(0, 0, 0, 9);
        s.set(4, 4, 4, 2);
        assert!(matches!(s, LightStorage::Dense(_)));
        assert_eq!(s.get(4, 4, 4), 2);
        assert_eq!(s.get(0, 0, 0), 9);
        assert_eq!(s.get(7, 8, 9), 5);
    }

    #[test]
    fn writing_a_shared_dense_storage_leaves_the_other_holder_alone() {
        let mut original = LightStorage::Uniform(5);
        original.set(0, 0, 0, 9);
        let shared = original.clone();
        original.set(1, 0, 0, 2);
        assert_eq!(original.get(1, 0, 0), 2);
        assert_eq!(shared.get(1, 0, 0), 5);
        assert_eq!(shared.get(0, 0, 0), 9);
    }

    #[test]
    fn get_returns_zero_on_null() {
        let s = LightStorage::Empty;
        assert_eq!(s.get(0, 0, 0), 0);
        assert_eq!(s.get(15, 15, 15), 0);
        assert_eq!(s.get(7, 3, 11), 0);
    }

    #[test]
    fn uniform_get_returns_constant_for_all_coords() {
        let s = LightStorage::Uniform(6);
        assert_eq!(s.get(0, 0, 0), 6);
        assert_eq!(s.get(15, 15, 15), 6);
        assert_eq!(s.get(1, 7, 3), 6);
    }
}
