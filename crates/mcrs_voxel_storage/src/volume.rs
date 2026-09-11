use bevy_math::{IVec3, UVec3};

use crate::VoxelId;

/// An axis-aligned box of positions, bounds inclusive.
pub trait Volume {
    fn min(&self) -> IVec3;

    fn max(&self) -> IVec3;

    #[inline]
    fn contains(&self, p: IVec3) -> bool {
        (self.min().cmple(p) & p.cmple(self.max())).all()
    }
}

/// A volume that answers a voxel per position. Outside its bounds the answer is
/// the default id, never a panic.
pub trait Blocks: Volume {
    fn get(&self, p: IVec3) -> VoxelId;

    fn iter(&self) -> impl Iterator<Item = (IVec3, VoxelId)> + '_ {
        let (a, b) = (self.min(), self.max());
        (a.y..=b.y).flat_map(move |y| {
            (a.z..=b.z).flat_map(move |z| {
                (a.x..=b.x).map(move |x| {
                    let p = IVec3::new(x, y, z);
                    (p, self.get(p))
                })
            })
        })
    }
}

/// A volume that takes writes. A write outside its bounds is dropped.
pub trait BlocksMut: Blocks {
    fn set(&mut self, p: IVec3, id: VoxelId);
}

/// An owned dense box: what a schematic is, and what a test world is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoxVolume {
    min: IVec3,
    size: UVec3,
    cells: Vec<VoxelId>,
}

impl BoxVolume {
    pub fn filled(min: IVec3, max: IVec3, id: VoxelId) -> Self {
        let size = (max - min + IVec3::ONE).max(IVec3::ZERO).as_uvec3();
        BoxVolume {
            min,
            size,
            cells: vec![id; (size.x * size.y * size.z) as usize],
        }
    }

    pub fn empty(min: IVec3, max: IVec3) -> Self {
        Self::filled(min, max, VoxelId::default())
    }

    #[inline]
    fn index(&self, p: IVec3) -> Option<usize> {
        if !self.contains(p) {
            return None;
        }
        let local = (p - self.min).as_uvec3();
        Some(((local.y * self.size.z + local.z) * self.size.x + local.x) as usize)
    }

    /// Fill a whole horizontal layer, clipped to the box.
    pub fn fill_layer(&mut self, y: i32, id: VoxelId) {
        let (a, b) = (self.min(), self.max());
        for z in a.z..=b.z {
            for x in a.x..=b.x {
                self.set(IVec3::new(x, y, z), id);
            }
        }
    }
}

impl Volume for BoxVolume {
    #[inline]
    fn min(&self) -> IVec3 {
        self.min
    }

    #[inline]
    fn max(&self) -> IVec3 {
        self.min + self.size.as_ivec3() - IVec3::ONE
    }
}

impl Blocks for BoxVolume {
    #[inline]
    fn get(&self, p: IVec3) -> VoxelId {
        self.index(p).map_or(VoxelId::default(), |i| self.cells[i])
    }
}

impl BlocksMut for BoxVolume {
    #[inline]
    fn set(&mut self, p: IVec3, id: VoxelId) {
        if let Some(i) = self.index(p) {
            self.cells[i] = id;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_reads_back_what_it_took_and_drops_the_rest() {
        let mut vol = BoxVolume::empty(IVec3::new(-2, 0, 5), IVec3::new(1, 3, 6));
        assert_eq!(vol.max(), IVec3::new(1, 3, 6));
        vol.set(IVec3::new(-2, 3, 6), VoxelId(7));
        vol.set(IVec3::new(2, 0, 5), VoxelId(9));
        assert_eq!(vol.get(IVec3::new(-2, 3, 6)), VoxelId(7));
        assert_eq!(vol.get(IVec3::new(2, 0, 5)), VoxelId::default());
        assert_eq!(vol.iter().count(), 4 * 4 * 2);
        assert_eq!(vol.iter().filter(|(_, id)| *id == VoxelId(7)).count(), 1);
    }
}
