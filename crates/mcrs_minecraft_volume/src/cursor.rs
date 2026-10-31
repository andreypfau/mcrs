use mcrs_voxel::BlockStateId;
use mcrs_voxel::{BlockPos, LocalPos, SectionPos};

use crate::access::Unavailable;
use crate::block::{BlockVolume, SectionBlocks};

/// Per-block reads that look a section up once and keep it while the reads stay in it.
pub struct Cursor<'v, V: BlockVolume + ?Sized + 'v> {
    volume: &'v V,
    last: Option<(SectionPos, Result<V::Section<'v>, Unavailable>)>,
}

impl<'v, V: BlockVolume + ?Sized> Cursor<'v, V> {
    pub fn new(volume: &'v V) -> Self {
        Self { volume, last: None }
    }

    pub fn block(&mut self, at: BlockPos) -> Result<BlockStateId, Unavailable> {
        if !self.volume.contains(at) {
            return Err(Unavailable::Outside);
        }
        let section = SectionPos::from(at);
        let cached = match &self.last {
            Some((held, cached)) if *held == section => cached,
            _ => {
                let volume = self.volume;
                &self.last.insert((section, volume.section(section))).1
            }
        };
        match cached {
            Ok(handle) => Ok(handle.get(LocalPos::from(at))),
            Err(why) => Err(*why),
        }
    }
}
