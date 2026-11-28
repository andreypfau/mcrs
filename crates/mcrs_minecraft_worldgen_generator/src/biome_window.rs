use crate::multi_noise_biomes::BiomeGrid;
use mcrs_minecraft_noise::sample_grid::SampleGrid;

#[derive(Clone, Copy)]
pub struct BiomeWindow<'a> {
    volume: SampleGrid,
    storage: SampleGrid,
    offset_x: i32,
    offset_z: i32,
    ids: &'a [u8],
}

impl<'a> BiomeWindow<'a> {
    pub(crate) fn new(
        volume: SampleGrid,
        storage: SampleGrid,
        offset_x: i32,
        offset_z: i32,
        ids: &'a [u8],
    ) -> Self {
        Self {
            volume,
            storage,
            offset_x,
            offset_z,
            ids,
        }
    }

    #[must_use]
    pub fn volume(&self) -> SampleGrid {
        self.volume
    }

    #[inline]
    #[must_use]
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        let at = self
            .storage
            .index_unchecked(x + self.offset_x, y, z + self.offset_z);
        self.ids.get(at).copied().unwrap_or_default()
    }

    #[must_use]
    pub fn to_grid(&self) -> BiomeGrid {
        let size = self.volume.size();
        let mut ids = Vec::with_capacity(self.volume.len());
        for z in 0..size.z {
            for x in 0..size.x {
                for y in 0..size.y {
                    ids.push(self.get(x, y, z));
                }
            }
        }
        BiomeGrid {
            volume: self.volume,
            ids,
        }
    }
}
