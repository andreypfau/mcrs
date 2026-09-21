use rustc_hash::FxHashMap as HashMap;

use std::sync::Arc;

use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_feature::placer::{Predicate, StateMask, WorldGenVolume};

use crate::holds;
use crate::tables::BlockTables;
use crate::tree::provider::StateProvider;

/// Which cap the same stem-and-scan skeleton carries: a flat plate one block
/// thick, or four layers with a rim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MushroomCap {
    Brown,
    Red,
}

/// `up`, `west`, `east`, `north`, `south` as bits 4 down to 0.
pub fn face_index(up: bool, west: bool, east: bool, north: bool, south: bool) -> usize {
    (up as usize) << 4
        | (west as usize) << 3
        | (east as usize) << 2
        | (north as usize) << 1
        | (south as usize)
}

/// The five `HugeMushroomBlock` faces of a cap state, as the states reached by
/// setting them, with the source's own `up` kept for the cap that never sets it.
///
/// A state whose block has no such faces is absent, and is written unchanged —
/// the reference's `hasProperty` guard.
#[derive(Clone, Debug, Default)]
pub struct MushroomFaces {
    entries: HashMap<u16, FaceEntry>,
}

#[derive(Clone, Copy, Debug)]
struct FaceEntry {
    up: bool,
    by_faces: [VoxelId; 32],
}

impl MushroomFaces {
    pub fn insert(&mut self, state: VoxelId, up: bool, by_faces: [VoxelId; 32]) {
        self.entries.insert(state.0, FaceEntry { up, by_faces });
    }

    /// `up` left `None` keeps the face the state already has.
    pub fn with_faces(
        &self,
        state: VoxelId,
        up: Option<bool>,
        west: bool,
        east: bool,
        north: bool,
        south: bool,
    ) -> VoxelId {
        match self.entries.get(&state.0) {
            Some(entry) => {
                entry.by_faces[face_index(up.unwrap_or(entry.up), west, east, north, south)]
            }
            None => state,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CompiledHugeMushroom {
    pub cap: MushroomCap,
    pub cap_provider: StateProvider,
    pub stem_provider: StateProvider,
    pub foliage_radius: i32,
    pub can_place_on: Predicate,
    /// `minecraft:leaves`, which the free-space scan tolerates but the writer
    /// does not.
    pub leaves: StateMask,
    pub replaceable_by_mushrooms: StateMask,
    pub tables: Arc<BlockTables>,
}

/// Both huge mushrooms: two height draws, a free-space scan that draws nothing,
/// then the cap and the stem, each spending whatever its state provider spends
/// per block it considers.
pub fn place_huge_mushroom<W: WorldGenVolume>(
    cfg: &CompiledHugeMushroom,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    at: BlockPos,
) -> bool {
    let mut height = rng.next_i32_bound(3) + 4;
    if rng.next_i32_bound(12) == 0 {
        height *= 2;
    }
    if !valid_position(cfg, volume, at, height) {
        return false;
    }
    match cfg.cap {
        MushroomCap::Brown => brown_cap(cfg, volume, rng, at, height),
        MushroomCap::Red => red_cap(cfg, volume, rng, at, height),
    }
    for dy in 0..height {
        let state = cfg.stem_provider.state(&*volume, rng, at);
        place_block(cfg, volume, at + IVec3::Y * dy, state);
    }
    true
}

impl CompiledHugeMushroom {
    fn scan_radius(&self, dy: i32, height: i32) -> i32 {
        match self.cap {
            MushroomCap::Brown => {
                if dy <= 3 {
                    0
                } else {
                    self.foliage_radius
                }
            }
            MushroomCap::Red => {
                if (dy < height && dy >= height - 3) || dy == height {
                    self.foliage_radius
                } else {
                    0
                }
            }
        }
    }
}

fn valid_position<W: WorldGenVolume>(
    cfg: &CompiledHugeMushroom,
    volume: &W,
    at: BlockPos,
    height: i32,
) -> bool {
    let extent = volume.extent();
    if at.y < extent.min_y + 1 || at.y + height + 1 > extent.min_y + extent.depth - 1 {
        return false;
    }
    if !cfg.can_place_on.test(volume, at - IVec3::Y) {
        return false;
    }
    for dy in 0..=height {
        let radius = cfg.scan_radius(dy, height);
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let state = volume.get(at + IVec3::new(dx, dy, dz));
                if !holds(&volume.world().air_states, state) && !holds(&cfg.leaves, state) {
                    return false;
                }
            }
        }
    }
    true
}

fn brown_cap<W: WorldGenVolume>(
    cfg: &CompiledHugeMushroom,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    at: BlockPos,
    height: i32,
) {
    let radius = cfg.foliage_radius;
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let (min_x, max_x) = (dx == -radius, dx == radius);
            let (min_z, max_z) = (dz == -radius, dz == radius);
            let x_edge = min_x || max_x;
            let z_edge = min_z || max_z;
            if x_edge && z_edge {
                continue;
            }
            let west = min_x || (z_edge && dx == 1 - radius);
            let east = max_x || (z_edge && dx == radius - 1);
            let north = min_z || (x_edge && dz == 1 - radius);
            let south = max_z || (x_edge && dz == radius - 1);
            let state = cfg.cap_provider.state(&*volume, rng, at);
            let state = cfg
                .tables
                .mushroom_faces
                .with_faces(state, None, west, east, north, south);
            place_block(cfg, volume, at + IVec3::new(dx, height, dz), state);
        }
    }
}

fn red_cap<W: WorldGenVolume>(
    cfg: &CompiledHugeMushroom,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    at: BlockPos,
    height: i32,
) {
    let center = cfg.foliage_radius - 2;
    for dy in height - 3..=height {
        let radius = if dy < height {
            cfg.foliage_radius
        } else {
            cfg.foliage_radius - 1
        };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let x_edge = dx == -radius || dx == radius;
                let z_edge = dz == -radius || dz == radius;
                if dy < height && x_edge == z_edge {
                    continue;
                }
                let state = cfg.cap_provider.state(&*volume, rng, at);
                let state = cfg.tables.mushroom_faces.with_faces(
                    state,
                    Some(dy >= height - 1),
                    dx < -center,
                    dx > center,
                    dz < -center,
                    dz > center,
                );
                place_block(cfg, volume, at + IVec3::new(dx, dy, dz), state);
            }
        }
    }
}

fn place_block<W: WorldGenVolume>(
    cfg: &CompiledHugeMushroom,
    volume: &mut W,
    pos: BlockPos,
    state: VoxelId,
) {
    let current = volume.get(pos);
    if holds(&volume.world().air_states, current) || holds(&cfg.replaceable_by_mushrooms, current) {
        volume.set(pos, state);
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_chunk::Blocks;
    use mcrs_minecraft_worldgen_feature::placer::mask_of;

    use mcrs_minecraft_random::worldgen::WorldgenRandom;

    use super::*;
    use crate::tree::provider::fake::FakeVolume;

    const DIRT: VoxelId = VoxelId(1);
    const STONE: VoxelId = VoxelId(2);
    const CAP: VoxelId = VoxelId(200);
    const STEM: VoxelId = VoxelId(300);
    const ORIGIN: BlockPos = BlockPos::new(8, 70, 8);

    /// The cap's 32 face states sit at `CAP + 1 ..= CAP + 32`, so a test reads
    /// the faces a placer asked for straight off the written id.
    fn faces() -> MushroomFaces {
        let mut table = MushroomFaces::default();
        let mut by_faces = [VoxelId(0); 32];
        for (index, slot) in by_faces.iter_mut().enumerate() {
            *slot = VoxelId(CAP.0 + 1 + index as u16);
        }
        table.insert(CAP, false, by_faces);
        table
    }

    fn config(cap: MushroomCap, foliage_radius: i32) -> CompiledHugeMushroom {
        CompiledHugeMushroom {
            cap,
            cap_provider: StateProvider::Simple(CAP),
            stem_provider: StateProvider::Simple(STEM),
            foliage_radius,
            can_place_on: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([DIRT.0]),
            },
            leaves: StateMask::default(),
            replaceable_by_mushrooms: StateMask::default(),
            tables: Arc::new(BlockTables {
                mushroom_faces: faces(),
                ..BlockTables::default()
            }),
        }
    }

    fn clearing() -> FakeVolume {
        let mut volume = FakeVolume::default();
        for x in ORIGIN.x - 6..=ORIGIN.x + 6 {
            for z in ORIGIN.z - 6..=ORIGIN.z + 6 {
                volume.blocks.insert((x, ORIGIN.y - 1, z), DIRT);
            }
        }
        volume
    }

    fn height_of(seed: u64) -> i32 {
        let mut rng = WorldgenRandom::new(seed);
        let mut height = rng.next_i32_bound(3) + 4;
        if rng.next_i32_bound(12) == 0 {
            height *= 2;
        }
        height
    }

    /// Two draws for the height and nothing else, because a
    /// `simple_state_provider` draws nothing: the cap is a plate at the top of
    /// the stem with its corners missing.
    #[test]
    fn a_brown_cap_is_a_cornerless_plate_over_its_stem() {
        let cfg = config(MushroomCap::Brown, 3);
        let mut volume = clearing();
        let mut rng = WorldgenRandom::new(0x00b2_0117);
        assert!(place_huge_mushroom(&cfg, &mut volume, &mut rng, ORIGIN));

        let mut replay = WorldgenRandom::new(0x00b2_0117);
        replay.next_i32_bound(3);
        replay.next_i32_bound(12);
        assert_eq!(rng, replay, "only the two height draws");

        let height = height_of(0x00b2_0117);
        let cap_cells = volume
            .writes
            .iter()
            .filter(|((_, y, _), _)| *y == ORIGIN.y + height)
            .count();
        assert_eq!(cap_cells, 7 * 7 - 4, "a seven-wide plate less its corners");
        for dy in 0..height {
            assert_eq!(volume.get(ORIGIN + IVec3::Y * dy), STEM);
        }
        assert_eq!(
            volume.get(ORIGIN + IVec3::new(-3, height, 0)),
            VoxelId(CAP.0 + 1 + face_index(false, true, false, false, false) as u16),
            "the west rim carries only its west face"
        );
    }

    /// The red cap is four layers: three rims below the top and a full plate at
    /// it, one narrower than the rims.
    #[test]
    fn a_red_cap_is_three_rims_under_a_plate() {
        let cfg = config(MushroomCap::Red, 2);
        let mut volume = clearing();
        let mut rng = WorldgenRandom::new(0x02ed_ca95);
        assert!(place_huge_mushroom(&cfg, &mut volume, &mut rng, ORIGIN));
        let height = height_of(0x02ed_ca95);

        for dy in height - 3..height {
            let ring = volume
                .writes
                .iter()
                .filter(|((x, y, z), _)| *y == ORIGIN.y + dy && (*x != ORIGIN.x || *z != ORIGIN.z))
                .count();
            assert_eq!(ring, 12, "a five-wide ring around the stem at dy {dy}");
        }
        let plate = volume
            .writes
            .iter()
            .filter(|((_, y, _), _)| *y == ORIGIN.y + height)
            .count();
        assert_eq!(plate, 9, "a three-wide plate at the top");
    }

    /// The height is drawn before anything is read, so a refused position costs
    /// the same two draws as a placed one.
    #[test]
    fn a_refused_position_still_spends_the_height_draws() {
        let cfg = config(MushroomCap::Red, 2);
        let mut volume = clearing();
        volume
            .blocks
            .insert((ORIGIN.x, ORIGIN.y - 1, ORIGIN.z), STONE);
        let mut rng = WorldgenRandom::new(5);
        assert!(!place_huge_mushroom(&cfg, &mut volume, &mut rng, ORIGIN));

        let mut replay = WorldgenRandom::new(5);
        replay.next_i32_bound(3);
        replay.next_i32_bound(12);
        assert_eq!(rng, replay);
        assert!(volume.writes.is_empty());
    }

    /// A block inside the cap's own scan radius refuses the whole mushroom.
    #[test]
    fn an_obstructed_crown_refuses() {
        let cfg = config(MushroomCap::Red, 2);
        let mut volume = clearing();
        let height = height_of(5);
        volume
            .blocks
            .insert((ORIGIN.x + 2, ORIGIN.y + height, ORIGIN.z), STONE);
        let mut rng = WorldgenRandom::new(5);
        assert!(!place_huge_mushroom(&cfg, &mut volume, &mut rng, ORIGIN));
        assert!(volume.writes.is_empty());
    }
}
