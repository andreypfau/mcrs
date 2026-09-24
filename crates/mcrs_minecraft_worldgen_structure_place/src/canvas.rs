use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, BoundingBox};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, FeatureCompileError};
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume, WorldStates};
use mcrs_minecraft_worldgen_feature_place::block_entity::{ContainerData, GeneratedBlockEntity};
use mcrs_minecraft_worldgen_feature_place::entity::GeneratedEntity;
use mcrs_minecraft_worldgen_feature_place::room::reorient;
use mcrs_minecraft_worldgen_structure::orient::{Orientation, world_pos};

use crate::{Oriented, block_mask, state};

/// The chest as `StructurePiece.createChest` reorients it, world-facing.
#[derive(Clone, Debug)]
pub struct ChestStates {
    /// Indexed like `Direction::HORIZONTAL`, north first.
    pub facing: [VoxelId; 4],
    pub states: StateMask,
}

impl ChestStates {
    pub fn compile(blocks: &dyn BlockResolver) -> Result<Self, FeatureCompileError> {
        let facing = |name| state(blocks, "minecraft:chest", &[("facing", name)]);
        Ok(ChestStates {
            facing: [
                facing("north")?,
                facing("east")?,
                facing("south")?,
                facing("west")?,
            ],
            states: block_mask(blocks, &["minecraft:chest"])?,
        })
    }
}

/// `StructurePiece.isReplaceableByStructures`: what `fillColumnDown` digs through.
pub fn replaceable_by_structures(
    blocks: &dyn BlockResolver,
    world: &WorldStates,
) -> Result<StateMask, FeatureCompileError> {
    let mut mask = fixedbitset::FixedBitSet::clone(&world.air_states);
    mask.union_with(&world.water_states);
    mask.union_with(&world.lava_states);
    let plants = block_mask(
        blocks,
        &[
            "minecraft:glow_lichen",
            "minecraft:seagrass",
            "minecraft:tall_seagrass",
        ],
    )?;
    mask.union_with(&plants);
    Ok(mask.into())
}

/// One grid piece's view of the column it is placing into: `StructurePiece`'s
/// drawing helpers over piece-local coordinates, every write clipped to the
/// column's writable box.
pub struct PieceCanvas<'a, W: WorldGenVolume> {
    pub volume: &'a mut W,
    pub entities: &'a mut Vec<GeneratedBlockEntity>,
    pub spawns: &'a mut Vec<GeneratedEntity>,
    pub bounds: BoundingBox,
    pub orientation: Option<Orientation>,
    pub clip: BoundingBox,
    /// States `place` never overwrites.
    pub keep: Option<&'a StateMask>,
}

impl<W: WorldGenVolume> PieceCanvas<'_, W> {
    pub fn world_pos(&self, x: i32, y: i32, z: i32) -> BlockPos {
        world_pos(self.orientation, self.bounds, IVec3::new(x, y, z))
    }

    fn oriented(&self, state: &Oriented) -> VoxelId {
        state.get(self.orientation.unwrap_or(Orientation::North))
    }

    /// `placeBlock`.
    pub fn place(&mut self, state: &Oriented, x: i32, y: i32, z: i32) {
        let pos = self.world_pos(x, y, z);
        if self.clip.is_inside(pos) && !self.keep.is_some_and(|keep| self.volume.holds(keep, pos)) {
            self.volume.set(pos, self.oriented(state));
        }
    }

    /// `getBlock`: air outside the clip.
    pub fn get(&self, x: i32, y: i32, z: i32) -> VoxelId {
        let pos = self.world_pos(x, y, z);
        if self.clip.is_inside(pos) {
            self.volume.get(pos)
        } else {
            self.volume.world().air
        }
    }

    pub(crate) fn is_air(&self, x: i32, y: i32, z: i32) -> bool {
        let state = self.get(x, y, z);
        self.volume.world().air_states.contains(state.0 as usize)
    }

    /// `generateBox`: `edge` on the six faces, `fill` inside.
    pub fn generate_box(
        &mut self,
        min: [i32; 3],
        max: [i32; 3],
        edge: &Oriented,
        fill: &Oriented,
        skip_air: bool,
    ) {
        self.generate_selected_box(
            min,
            max,
            skip_air,
            |on_face| {
                if on_face { edge } else { fill }
            },
        );
    }

    /// `generateBox` with one state throughout, air included.
    pub fn solid(&mut self, state: &Oriented, min: [i32; 3], max: [i32; 3]) {
        self.generate_box(min, max, state, state, false);
    }

    /// `generateBox` with a `BlockSelector`: `select` is asked once per cell,
    /// told whether the cell lies on the six faces, before the clip is tested.
    pub fn generate_selected_box<'s>(
        &mut self,
        min: [i32; 3],
        max: [i32; 3],
        skip_air: bool,
        mut select: impl FnMut(bool) -> &'s Oriented,
    ) {
        let [x0, y0, z0] = min;
        let [x1, y1, z1] = max;
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if skip_air && self.is_air(x, y, z) {
                        continue;
                    }
                    let edge = y == y0 || y == y1 || x == x0 || x == x1 || z == z0 || z == z1;
                    self.place(select(edge), x, y, z);
                }
            }
        }
    }

    /// `generateMaybeBox`: one float per cell, drawn before any test.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_maybe_box<R: Random>(
        &mut self,
        rng: &mut R,
        probability: f32,
        min: [i32; 3],
        max: [i32; 3],
        edge: &Oriented,
        fill: &Oriented,
        skip_air: bool,
        has_to_be_inside: bool,
    ) {
        let [x0, y0, z0] = min;
        let [x1, y1, z1] = max;
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if rng.next_f32() > probability
                        || (skip_air && self.is_air(x, y, z))
                        || (has_to_be_inside && !self.is_interior(x, y, z))
                    {
                        continue;
                    }
                    let inside = y != y0 && y != y1 && x != x0 && x != x1 && z != z0 && z != z1;
                    self.place(if inside { fill } else { edge }, x, y, z);
                }
            }
        }
    }

    /// `maybeGenerateBlock`.
    pub fn maybe_generate_block<R: Random>(
        &mut self,
        rng: &mut R,
        probability: f32,
        x: i32,
        y: i32,
        z: i32,
        state: &Oriented,
    ) {
        if rng.next_f32() < probability {
            self.place(state, x, y, z);
        }
    }

    /// `generateUpperHalfSphere`.
    pub fn upper_half_sphere(
        &mut self,
        min: [i32; 3],
        max: [i32; 3],
        fill: &Oriented,
        skip_air: bool,
    ) {
        let [x0, y0, z0] = min;
        let [x1, y1, z1] = max;
        let diag_x = (x1 - x0 + 1) as f32;
        let diag_y = (y1 - y0 + 1) as f32;
        let diag_z = (z1 - z0 + 1) as f32;
        let centre_x = x0 as f32 + diag_x / 2.0;
        let centre_z = z0 as f32 + diag_z / 2.0;
        for y in y0..=y1 {
            let dy = (y - y0) as f32 / diag_y;
            for x in x0..=x1 {
                let dx = (x as f32 - centre_x) / (diag_x * 0.5);
                for z in z0..=z1 {
                    let dz = (z as f32 - centre_z) / (diag_z * 0.5);
                    if skip_air && self.is_air(x, y, z) {
                        continue;
                    }
                    if dx * dx + dy * dy + dz * dz <= 1.05 {
                        self.place(fill, x, y, z);
                    }
                }
            }
        }
    }

    /// `fillColumnDown`: from `start_y` down through everything in
    /// `replaceable`, stopping one above the dimension floor; the state is
    /// written as given, unoriented, as the reference's `setBlock` does.
    pub fn fill_column_down(
        &mut self,
        replaceable: &StateMask,
        state: VoxelId,
        x: i32,
        start_y: i32,
        z: i32,
    ) {
        let mut pos = self.world_pos(x, start_y, z);
        if !self.clip.is_inside(pos) {
            return;
        }
        let floor = self.volume.extent().min_y + 1;
        while self.volume.holds(replaceable, pos) && pos.y > floor {
            self.volume.set(pos, state);
            pos.y -= 1;
        }
    }

    /// `isInterior`: the cell above lies inside the clip and under the
    /// ocean-floor height.
    pub fn is_interior(&self, x: i32, y: i32, z: i32) -> bool {
        let pos = self.world_pos(x, y + 1, z);
        self.clip.is_inside(pos)
            && pos.y
                < self
                    .volume
                    .height(HeightmapName::OceanFloorWg, pos.x, pos.z)
    }

    /// `createChest`: whether one was placed, which is when its loot seed is drawn.
    pub fn create_chest<R: Random>(
        &mut self,
        rng: &mut R,
        chest: &ChestStates,
        x: i32,
        y: i32,
        z: i32,
        loot_table: &str,
    ) -> bool {
        let pos = self.world_pos(x, y, z);
        if !self.clip.is_inside(pos) || self.volume.holds(&chest.states, pos) {
            return false;
        }
        let state = reorient(&chest.facing, &chest.states, self.volume, pos);
        self.volume.set(pos, state);
        self.entities.push(GeneratedBlockEntity::chest(
            pos,
            loot_table.to_owned(),
            rng.next_java_long(),
        ));
        true
    }

    /// `createDispenser`, with the dispenser already facing as the caller asked.
    #[allow(clippy::too_many_arguments)]
    pub fn create_dispenser<R: Random>(
        &mut self,
        rng: &mut R,
        dispenser: &Oriented,
        dispenser_states: &StateMask,
        x: i32,
        y: i32,
        z: i32,
        loot_table: &str,
    ) -> bool {
        let pos = self.world_pos(x, y, z);
        if !self.clip.is_inside(pos) || self.volume.holds(dispenser_states, pos) {
            return false;
        }
        self.place(dispenser, x, y, z);
        self.entities
            .push(GeneratedBlockEntity::Dispenser(ContainerData::looted(
                pos,
                loot_table.to_owned(),
                rng.next_java_long(),
            )));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_random::legacy::LegacyRandom;
    use mcrs_minecraft_worldgen_feature::placer::{BoxRegion, mask_of};

    const AIR: VoxelId = VoxelId(0);
    const STONE: VoxelId = VoxelId(1);
    const BRICK: VoxelId = VoxelId(2);

    fn region() -> BoxRegion {
        let mut region = BoxRegion::new(BlockPos::new(0, 0, 0), BlockPos::new(31, 31, 31), AIR)
            .floor(9, STONE)
            .with_height(|_, _, _, _| 10);
        region.world.air_states = mask_of([AIR]);
        region
    }

    fn canvas<'a>(
        region: &'a mut BoxRegion,
        entities: &'a mut Vec<GeneratedBlockEntity>,
        spawns: &'a mut Vec<GeneratedEntity>,
        orientation: Orientation,
    ) -> PieceCanvas<'a, BoxRegion> {
        PieceCanvas {
            volume: region,
            entities,
            spawns,
            bounds: BoundingBox {
                min: BlockPos::new(8, 10, 8),
                max: BlockPos::new(23, 20, 23),
            },
            orientation: Some(orientation),
            clip: BoundingBox {
                min: BlockPos::new(0, 1, 0),
                max: BlockPos::new(15, 31, 31),
            },
            keep: None,
        }
    }

    #[test]
    fn writes_are_oriented_and_clipped() {
        let mut region = region();
        let mut entities = Vec::new();
        let mut spawns = Vec::new();
        let brick = Oriented([BRICK; 4]);
        let mut east = canvas(&mut region, &mut entities, &mut spawns, Orientation::East);
        east.generate_box([0, 0, 0], [3, 0, 3], &brick, &brick, false);
        let writes = region.writes.len();
        assert_eq!(
            writes, 16,
            "the east box lands in x 8..=11, inside the clip"
        );
        let mut north = canvas(&mut region, &mut entities, &mut spawns, Orientation::North);
        north.generate_box([10, 0, 0], [13, 0, 3], &brick, &brick, false);
        assert_eq!(region.writes.len(), writes, "x 18..=21 is past the clip");
    }

    #[test]
    fn the_column_fills_down_through_air_only() {
        let mut region = region();
        let mut entities = Vec::new();
        let mut spawns = Vec::new();
        let mut canvas = canvas(&mut region, &mut entities, &mut spawns, Orientation::North);
        canvas.fill_column_down(&mask_of([AIR]), BRICK, 2, 5, 2);
        let column: Vec<i32> = region.writes.iter().map(|(pos, _)| pos.y).collect();
        assert_eq!(column, vec![15, 14, 13, 12, 11, 10]);
    }

    #[test]
    fn interior_and_sphere_follow_the_reference_shape() {
        let mut region = region();
        let mut entities = Vec::new();
        let mut spawns = Vec::new();
        let brick = Oriented([BRICK; 4]);
        let mut canvas = canvas(&mut region, &mut entities, &mut spawns, Orientation::North);
        assert!(
            !canvas.is_interior(0, 0, 0),
            "y 11 is above the height of 10"
        );
        assert!(canvas.is_interior(0, -3, 0));
        canvas.upper_half_sphere([0, 0, 0], [4, 4, 4], &brick, false);
        assert_eq!(region.writes.len(), 76);
    }

    #[test]
    fn the_maybe_box_draws_once_per_cell() {
        let mut region = region();
        let mut entities = Vec::new();
        let brick = Oriented([BRICK; 4]);
        let mut rng = LegacyRandom::new(3);
        let mut replay = rng.clone();
        let mut spawns = Vec::new();
        let mut canvas = canvas(&mut region, &mut entities, &mut spawns, Orientation::South);
        canvas.generate_maybe_box(
            &mut rng,
            0.5,
            [0, 0, 0],
            [2, 1, 2],
            &brick,
            &brick,
            false,
            true,
        );
        for _ in 0..18 {
            replay.next_f32();
        }
        assert_eq!(rng, replay);
        assert!(
            region.writes.is_empty(),
            "nothing above the height is interior"
        );
    }
}
