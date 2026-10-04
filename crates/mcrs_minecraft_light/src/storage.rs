use mcrs_minecraft_core::SectionPos;
use std::sync::Arc;

use mcrs_minecraft_chunk::SectionNibbles;

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
    pub fn from_field(cells: &[u8; SectionPos::VOLUME]) -> Self {
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
        for (byte, pair) in arr.0.iter_mut().zip(cells.as_chunks::<2>().0) {
            *byte = (pair[0] & 0x0F) | ((pair[1] & 0x0F) << 4);
        }
        LightStorage::Dense(Arc::new(arr))
    }

    /// Whether a working field already holds what this storage does.
    ///
    /// Answering without packing is the point: most sections of an epoch's field
    /// come back with the light they went in with, and packing two kilobytes and
    /// allocating an `Arc` to discover that is what reading a section back costs.
    pub fn matches_field(&self, cells: &[u8; SectionPos::VOLUME]) -> bool {
        match self {
            LightStorage::Empty => cells.iter().all(|&cell| cell & 0x0F == 0),
            LightStorage::Uniform(value) => cells.iter().all(|&cell| cell & 0x0F == *value),
            LightStorage::Dense(packed) => packed
                .0
                .iter()
                .zip(cells.as_chunks::<2>().0)
                .all(|(byte, pair)| *byte == (pair[0] & 0x0F) | ((pair[1] & 0x0F) << 4)),
        }
    }

    pub fn write_field(&self, cells: &mut [u8; SectionPos::VOLUME]) {
        match self {
            LightStorage::Empty => cells.fill(0),
            LightStorage::Uniform(v) => cells.fill(*v),
            LightStorage::Dense(arr) => {
                for (byte, pair) in arr.0.iter().zip(cells.as_chunks_mut::<2>().0) {
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

    fn field_of(f: impl Fn(usize) -> u8) -> Box<[u8; SectionPos::VOLUME]> {
        let mut cells = Box::new([0u8; SectionPos::VOLUME]);
        for (i, cell) in cells.iter_mut().enumerate() {
            *cell = f(i);
        }
        cells
    }

    #[test]
    fn a_field_normalizes_to_the_smallest_storage_and_round_trips() {
        assert!(matches!(
            LightStorage::from_field(&field_of(|_| 0)),
            LightStorage::Empty
        ));
        assert!(matches!(
            LightStorage::from_field(&field_of(|_| 12)),
            LightStorage::Uniform(12)
        ));

        let cells = field_of(|i| (i % 16) as u8);
        let storage = LightStorage::from_field(&cells);
        assert!(matches!(storage, LightStorage::Dense(_)));
        assert_eq!(
            storage.get(3, 7, 11),
            cells[SectionNibbles::index(3, 7, 11)]
        );
        let mut back = Box::new([0u8; SectionPos::VOLUME]);
        storage.write_field(&mut back);
        assert_eq!(&back[..], &cells[..]);

        LightStorage::Empty.write_field(&mut back);
        assert!(back.iter().all(|&c| c == 0));
        LightStorage::Uniform(4).write_field(&mut back);
        assert!(back.iter().all(|&c| c == 4));
    }

    #[test]
    fn a_set_densifies_and_leaves_a_shared_copy_alone() {
        let mut original = LightStorage::Uniform(5);
        original.set(0, 0, 0, 9);
        assert!(matches!(original, LightStorage::Dense(_)));
        let shared = original.clone();
        original.set(1, 0, 0, 2);
        assert_eq!(original.get(1, 0, 0), 2);
        assert_eq!(original.get(7, 8, 9), 5);
        assert_eq!(shared.get(1, 0, 0), 5);
        assert_eq!(shared.get(0, 0, 0), 9);
    }
}
