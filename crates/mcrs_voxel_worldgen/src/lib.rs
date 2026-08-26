use bevy_ecs::resource::Resource;
use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_math::{ChunkPos, ColumnPos};
use mcrs_voxel_storage::{VoxelId, VoxelPalette};
use rustc_hash::FxHashMap;
use std::future::Future;
use std::sync::Arc;

pub type SectionVoxels = VoxelPalette<VoxelId, { BLOCKS::SIZE }>;

/// One section a generator filled, at the Y it belongs to.
pub struct GeneratedSection {
    pub chunk_y: i32,
    pub voxels: SectionVoxels,
}

/// What a generator hands back for one column.
#[derive(Default)]
pub struct GeneratedChunk {
    pub sections: Vec<GeneratedSection>,
}

/// The buffer a generator fills. Section-addressed so a generator can skip a Y
/// range entirely rather than allocating it.
#[derive(Default)]
pub struct ChunkBuffer {
    sections: FxHashMap<i32, SectionVoxels>,
}

impl ChunkBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn section_mut(&mut self, chunk_y: i32) -> &mut SectionVoxels {
        self.sections.entry(chunk_y).or_default()
    }

    /// `y` is absolute; `x` and `z` are section-local.
    pub fn set_voxel(&mut self, x: usize, y: i32, z: usize, id: VoxelId) {
        let chunk_y = y >> BLOCKS::BITS;
        let local_y = (y as usize) & BLOCKS::MASK;
        self.section_mut(chunk_y).set_cell(x, local_y, z, id);
    }

    pub fn fill_section(&mut self, chunk_y: i32, id: VoxelId) {
        self.section_mut(chunk_y).fill(id);
    }

    pub fn finish(self) -> GeneratedChunk {
        let mut sections: Vec<GeneratedSection> = self
            .sections
            .into_iter()
            .map(|(chunk_y, voxels)| GeneratedSection { chunk_y, voxels })
            .collect();
        sections.sort_by_key(|s| s.chunk_y);
        GeneratedChunk { sections }
    }
}

/// Which sections the caller still wants. A generator may fill fewer, never more.
#[derive(Clone, Debug)]
pub struct StillNeeded(pub Vec<i32>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Background,
    Nearby,
    Blocking,
}

/// The engine's entire knowledge of world generation.
pub trait Generator: Send + Sync + 'static {
    fn generate(
        &self,
        seed: u64,
        pos: ColumnPos,
        still_needed: StillNeeded,
        priority: Priority,
    ) -> impl Future<Output = GeneratedChunk> + Send;
}

/// Key-addressed registry of generator providers. A game registers what it has;
/// the engine only looks one up by key.
#[derive(Resource, Default)]
pub struct GeneratorRegistry<G: ?Sized> {
    providers: FxHashMap<String, Arc<G>>,
}

impl<G: ?Sized> GeneratorRegistry<G> {
    pub fn register(&mut self, key: impl Into<String>, provider: Arc<G>) {
        self.providers.insert(key.into(), provider);
    }

    pub fn get(&self, key: &str) -> Option<&Arc<G>> {
        self.providers.get(key)
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

/// Per-position derivation is stateless hashing of the seed and the position;
/// nothing caches a seeded generator as world state.
pub fn derive_seed(seed: u64, pos: ChunkPos, domain: u64) -> u64 {
    let mut h = seed ^ domain.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for v in [
        pos.x as i64 as u64,
        pos.y as i64 as u64,
        pos.z as i64 as u64,
    ] {
        h ^= v.wrapping_add(0x9E37_79B9_7F4A_7C15);
        h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        h ^= h >> 33;
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_voxel_routes_absolute_y_to_its_section() {
        let mut buf = ChunkBuffer::new();
        buf.set_voxel(1, -1, 2, VoxelId(7));
        buf.set_voxel(1, 16, 2, VoxelId(9));
        let out = buf.finish();
        let ys: Vec<i32> = out.sections.iter().map(|s| s.chunk_y).collect();
        assert_eq!(ys, vec![-1, 1], "sections come back ascending by Y");
        assert_eq!(out.sections[0].voxels.0.get(1, 15, 2), VoxelId(7));
        assert_eq!(out.sections[1].voxels.0.get(1, 0, 2), VoxelId(9));
    }

    #[test]
    fn a_generator_fills_only_the_sections_it_touched() {
        let mut buf = ChunkBuffer::new();
        buf.fill_section(3, VoxelId(1));
        assert_eq!(buf.finish().sections.len(), 1);
    }

    #[test]
    fn seed_derivation_is_stateless_and_position_dependent() {
        let a = derive_seed(42, ChunkPos::new(1, 2, 3), 0);
        assert_eq!(a, derive_seed(42, ChunkPos::new(1, 2, 3), 0));
        assert_ne!(a, derive_seed(42, ChunkPos::new(1, 2, 4), 0));
        assert_ne!(a, derive_seed(42, ChunkPos::new(1, 2, 3), 1));
        assert_ne!(a, derive_seed(43, ChunkPos::new(1, 2, 3), 0));
    }
}
