#![allow(dead_code)]

use std::sync::{Arc, OnceLock};

use bevy_math::Vec3;
use mcrs_minecraft_chunk::{VoxelId, VoxelPalette};
use mcrs_minecraft_core::voxel_shape::{Aabb, VoxelShape};
use mcrs_minecraft_core::{BlockPos, BoundingBox, LocalPos, SectionPos};
use mcrs_minecraft_light::field::{BlockSnapshot, FieldLayout, LightField, SectionSource};
use mcrs_minecraft_light::prelude::*;
use mcrs_minecraft_light::relax;
use mcrs_minecraft_light_color::colors::{LightColors, LightType};
use mcrs_minecraft_light_color::propagate::{Lanes, propagate};
use mcrs_minecraft_light_color::region::{
    EdgeCosts, Palette, Region, Seed, section_bricks, section_output,
};
use proptest::prelude::*;

pub const AIR: VoxelId = VoxelId(0);
pub const STONE: VoxelId = VoxelId(1);
pub const GLASS: VoxelId = VoxelId(2);
pub const WATER: VoxelId = VoxelId(3);
pub const LEAVES: VoxelId = VoxelId(4);
pub const BOTTOM_SLAB: VoxelId = VoxelId(5);
pub const TOP_SLAB: VoxelId = VoxelId(6);
pub const STAIRS: VoxelId = VoxelId(7);
pub const GLOWSTONE: VoxelId = VoxelId(8);
pub const TORCH: VoxelId = VoxelId(9);
pub const SOUL: VoxelId = VoxelId(10);
pub const REDSTONE: VoxelId = VoxelId(11);
pub const LAVA: VoxelId = VoxelId(12);
pub const PALETTE_COUNT: u16 = 40;
pub const UNLOADED: VoxelId = VoxelId(13 + PALETTE_COUNT);
pub const OUTSIDE: VoxelId = VoxelId(14 + PALETTE_COUNT);

pub const TORCH_TYPE: LightType = LightType(1);
pub const SOUL_TYPE: LightType = LightType(2);
pub const REDSTONE_TYPE: LightType = LightType(3);
pub const LAVA_TYPE: LightType = LightType(4);
pub const COLOUR_COUNT: u8 = 4 + PALETTE_COUNT as u8;

pub fn palette_block(i: u16) -> VoxelId {
    assert!(i < PALETTE_COUNT);
    VoxelId(13 + i)
}

const LOWER_HALF: Aabb = Aabb {
    min: Vec3::ZERO,
    max: Vec3::new(1.0, 0.5, 1.0),
};
const UPPER_HALF: Aabb = Aabb {
    min: Vec3::new(0.0, 0.5, 0.0),
    max: Vec3::ONE,
};
const UPPER_QUARTER: Aabb = Aabb {
    min: Vec3::new(0.0, 0.5, 0.0),
    max: Vec3::new(0.5, 1.0, 1.0),
};

fn shape(boxes: &[Aabb]) -> &'static VoxelShape {
    Box::leak(Box::new(VoxelShape::from_boxes(boxes)))
}

fn shapes() -> &'static [&'static VoxelShape; 3] {
    static SHAPES: OnceLock<[&'static VoxelShape; 3]> = OnceLock::new();
    SHAPES.get_or_init(|| {
        [
            shape(&[LOWER_HALF]),
            shape(&[UPPER_HALF]),
            shape(&[LOWER_HALF, UPPER_QUARTER]),
        ]
    })
}

pub fn registry() -> Arc<LightRegistry> {
    static REGISTRY: OnceLock<Arc<LightRegistry>> = OnceLock::new();
    REGISTRY
        .get_or_init(|| {
            let [bottom, top, stairs] = *shapes();
            let mut properties = vec![
                LightProperties::AIR,
                LightProperties::SOLID,
                LightProperties::transparent(0),
                LightProperties::transparent(1),
                LightProperties::transparent(1),
                LightProperties::shaped(0, bottom),
                LightProperties::shaped(0, top),
                LightProperties::shaped(0, stairs),
                LightProperties {
                    dampening: 15,
                    emission: LightLevel::new(15),
                    occlusion: None,
                },
                LightProperties::emitter(14),
                LightProperties::emitter(10),
                LightProperties::emitter(7),
                LightProperties::emitter(15),
            ];
            properties
                .extend((0..PALETTE_COUNT).map(|i| LightProperties::emitter(1 + (i % 15) as u8)));
            properties.push(LightProperties::SOLID);
            properties.push(LightProperties::AIR);
            Arc::new(LightRegistry::new(
                properties,
                SpecialBlocks {
                    unloaded: UNLOADED,
                    outside: OUTSIDE,
                },
            ))
        })
        .clone()
}

pub fn colour(t: LightType) -> [u8; 3] {
    let i = t.0 as u32;
    [(i * 5) as u8, (255 - i * 3) as u8, (i * 97 % 256) as u8]
}

pub fn colours() -> LightColors {
    let table: Vec<[u8; 3]> = (1..=COLOUR_COUNT).map(|t| colour(LightType(t))).collect();
    let mut types = vec![LightType::DEFAULT; OUTSIDE.0 as usize + 1];
    types[TORCH.0 as usize] = TORCH_TYPE;
    types[SOUL.0 as usize] = SOUL_TYPE;
    types[REDSTONE.0 as usize] = REDSTONE_TYPE;
    types[LAVA.0 as usize] = LAVA_TYPE;
    for i in 0..PALETTE_COUNT {
        types[palette_block(i).0 as usize] = LightType(5 + i as u8);
    }
    LightColors::new(table, types)
}

/// The 3×3×3 sections around `centre`. A section outside `bounds` is always
/// `None`; inside, `None` means not loaded.
pub struct Neighbourhood {
    pub centre: SectionPos,
    pub bounds: LightBounds,
    pub sections: Vec<Option<Box<[u16; SectionPos::VOLUME]>>>,
}

impl Neighbourhood {
    pub fn filled(
        centre: SectionPos,
        bounds: LightBounds,
        fill: impl Fn(SectionPos) -> Option<VoxelId>,
    ) -> Self {
        let sections = neighbours(centre)
            .map(|pos| {
                if pos.y < bounds.min_section_y || pos.y > bounds.max_section_y {
                    return None;
                }
                fill(pos).map(|id| Box::new([id.0; SectionPos::VOLUME]))
            })
            .collect();
        Self {
            centre,
            bounds,
            sections,
        }
    }

    pub fn air(centre: SectionPos) -> Self {
        Self::filled(centre, LightBounds::new(centre.y - 4, centre.y + 4), |_| {
            Some(AIR)
        })
    }

    fn slot(&self, pos: SectionPos) -> Option<usize> {
        let d = pos.0 - self.centre.0 + 1;
        (d.cmpge(bevy_math::IVec3::ZERO).all() && d.cmplt(bevy_math::IVec3::splat(3)).all())
            .then(|| (d.x + 3 * d.z + 9 * d.y) as usize)
    }

    pub fn section(&self, pos: SectionPos) -> Option<&[u16; SectionPos::VOLUME]> {
        self.slot(pos).and_then(|i| self.sections[i].as_deref())
    }

    /// Writes into a loaded section; a position in a missing section is left alone.
    pub fn set(&mut self, pos: BlockPos, id: VoxelId) {
        if let Some(i) = self.slot(SectionPos::from(pos))
            && let Some(cells) = &mut self.sections[i]
        {
            cells[LocalPos::from(pos).index()] = id.0;
        }
    }

    pub fn cells<'a>(
        &'a self,
    ) -> impl Fn(SectionPos) -> Option<&'a [u16; SectionPos::VOLUME]> + 'a {
        |pos| self.section(pos)
    }

    pub fn block_min(&self) -> BlockPos {
        let s = SectionPos::SIZE as i32;
        BlockPos::new(
            (self.centre.x - 1) * s,
            (self.centre.y - 1) * s,
            (self.centre.z - 1) * s,
        )
    }
}

fn neighbours(centre: SectionPos) -> impl Iterator<Item = SectionPos> {
    (-1..=1).flat_map(move |dy| {
        (-1..=1).flat_map(move |dz| {
            (-1..=1).map(move |dx| SectionPos::new(centre.x + dx, centre.y + dy, centre.z + dz))
        })
    })
}

/// Every output position of the centre section, x fastest then z then y.
pub fn output_positions(centre: SectionPos) -> impl Iterator<Item = BlockPos> {
    let (min, size) = section_output(centre);
    (0..size).flat_map(move |y| {
        (0..size).flat_map(move |z| {
            (0..size).map(move |x| BlockPos::new(min.x + x, min.y + y, min.z + z))
        })
    })
}

/// The first cell where a brick of a section around `centre` differs from the
/// edge costs or sources of the centre's region. Costs are compared where the
/// cell and its six neighbours lie inside the region, sources wherever the
/// cell does.
pub fn brick_mismatch<'a>(
    centre: SectionPos,
    bounds: LightBounds,
    registry: &LightRegistry,
    colours: &LightColors,
    cells: impl Fn(SectionPos) -> Option<&'a [u16; SectionPos::VOLUME]>,
) -> Option<String> {
    let (min, size) = section_output(centre);
    let region = Region::new(min, size, bounds, registry, &cells);
    let costs = EdgeCosts::new(&region, registry);
    let width = SectionPos::SIZE as i32;
    neighbours(centre).find_map(|section| {
        let bricks = section_bricks(section, bounds, registry, colours, &cells);
        LocalPos::all().find_map(|local| {
            let at = [
                section.x * width + local.x() as i32 - region.min.x,
                section.y * width + local.y() as i32 - region.min.y,
                section.z * width + local.z() as i32 - region.min.z,
            ];
            if at.iter().any(|&d| d < 0 || d >= region.size) {
                return None;
            }
            let (i, cell) = (local.index(), region.index(at[0], at[1], at[2]));
            let id = region.blocks[cell];
            let emission = registry.emission(id);
            let seed = match emission.is_zero() {
                true => Seed::default(),
                false => Seed {
                    light_type: colours.light_type(id),
                    emission: emission.get(),
                },
            };
            let interior = at.iter().all(|&d| d > 0 && d + 1 < region.size);
            let got = (bricks.entry[i], bricks.veto[i]);
            let want = (costs.entry[cell], costs.veto[cell]);
            (bricks.seeds[i] != seed || interior && got != want).then(|| {
                format!(
                    "{section:?} cell {i}: brick {got:?} {:?}, region {want:?} {seed:?}",
                    bricks.seeds[i]
                )
            })
        })
    })
}

pub fn torch_on_a_floor(centre: SectionPos) -> Neighbourhood {
    let mut world = Neighbourhood::air(centre);
    let floor = centre.y * 16 + 2;
    for z in -16..32 {
        for x in -16..32 {
            world.set(BlockPos::new(x, floor, z), STONE);
        }
    }
    world.set(BlockPos::new(5, floor + 1, 5), TORCH);
    world.set(BlockPos::new(6, floor + 1, 5), GLASS);
    world
}

pub fn many_types(centre: SectionPos, count: u16) -> Neighbourhood {
    let mut world = Neighbourhood::air(centre);
    let base = centre.y * 16;
    for x in -16..32 {
        for z in -16..32 {
            world.set(BlockPos::new(x, base + 3, z), STONE);
        }
        world.set(BlockPos::new(x, base + 8, 6), WATER);
        world.set(BlockPos::new(x, base + 8, 7), LEAVES);
    }
    for i in 0..count {
        let (x, z) = ((i % 7) as i32 * 2 + 1, (i / 7) as i32 * 2 + 1);
        world.set(
            BlockPos::new(x, base + 4 + (i % 3) as i32, z),
            palette_block(i),
        );
    }
    world
}

pub fn soul_and_lava_by_a_wall(centre: SectionPos) -> Neighbourhood {
    let mut world = Neighbourhood::air(centre);
    let base = centre.y * 16;
    for y in base..base + 16 {
        for z in -16..32 {
            world.set(BlockPos::new(7, y, z), STONE);
        }
    }
    world.set(BlockPos::new(7, base + 5, 5), BOTTOM_SLAB);
    world.set(BlockPos::new(3, base + 5, 5), SOUL);
    world.set(BlockPos::new(12, base + 2, 9), LAVA);
    world
}

pub fn glowstone_among_torches(centre: SectionPos) -> (Neighbourhood, BlockPos) {
    let mut world = Neighbourhood::air(centre);
    let glowstone = BlockPos::new(8, centre.y * 16 + 8, 8);
    world.set(glowstone, GLOWSTONE);
    for dir in mcrs_minecraft_core::Direction::all() {
        world.set(glowstone + dir.normal(), TORCH);
    }
    (world, glowstone)
}

pub fn unknown_blocks_by_a_torch(centre: SectionPos) -> (Neighbourhood, BlockPos) {
    let mut world = torch_on_a_floor(centre);
    let unknown = VoxelId(u16::MAX - 1);
    let torch = BlockPos::new(5, centre.y * 16 + 3, 5);
    let beside = BlockPos::new(4, torch.y, 5);
    world.set(beside, unknown);
    world.set(BlockPos::new(5, torch.y + 1, 5), unknown);
    (world, beside)
}

fn placed_block() -> impl Strategy<Value = VoxelId> {
    let weighted = [
        (AIR, 10),
        (STONE, 10),
        (GLASS, 10),
        (WATER, 10),
        (LEAVES, 10),
        (BOTTOM_SLAB, 10),
        (TOP_SLAB, 10),
        (STAIRS, 10),
        (GLOWSTONE, 1),
        (TORCH, 1),
        (SOUL, 1),
        (REDSTONE, 1),
        (LAVA, 1),
    ];
    let pool: Vec<VoxelId> = weighted
        .iter()
        .flat_map(|&(block, weight)| std::iter::repeat_n(block, weight))
        .collect();
    prop::sample::select(pool)
}

/// Where the world's bounds cut the neighbourhood, the base fill of the centre
/// and of each of the other 26 sections, and the blocks placed over it.
pub type WorldParts = (u8, u8, Vec<u8>, Vec<(i32, i32, i32, VoxelId)>);

pub fn world_parts() -> impl Strategy<Value = WorldParts> {
    (
        0..3u8,
        0..4u8,
        prop::collection::vec(0..5u8, 26),
        prop::collection::vec((0..48i32, 0..48i32, 0..48i32, placed_block()), 0..=1500),
    )
}

pub fn random_world(centre: SectionPos, parts: WorldParts) -> Neighbourhood {
    let (place, centre_fill, fills, placements) = parts;
    let bounds = match place {
        0 => LightBounds::new(centre.y, centre.y + 6),
        1 => LightBounds::new(centre.y - 6, centre.y),
        _ => LightBounds::new(centre.y - 6, centre.y + 6),
    };
    let mut kinds = fills;
    kinds.insert(13, centre_fill);
    let mut world = Neighbourhood::filled(centre, bounds, |pos| {
        let d = pos.0 - centre.0 + 1;
        [Some(AIR), Some(STONE), Some(WATER), Some(LEAVES), None]
            [kinds[(d.x + 3 * d.z + 9 * d.y) as usize] as usize]
    });
    let min = world.block_min();
    for (x, y, z, block) in placements {
        world.set(BlockPos::new(min.x + x, min.y + y, min.z + z), block);
    }
    world
}

/// Light of type `t` on the centre section's output, from the server's relax
/// seeded with that type's emitters only.
pub fn oracle(neighbourhood: &Neighbourhood, t: LightType) -> Vec<u8> {
    let registry = registry();
    let colours = colours();
    let centre = neighbourhood.centre;
    let area = BoundingBox::of_section(centre).inflated(SectionPos::SIZE as i32);
    let layout = FieldLayout::covering(area);
    let sources = layout
        .sections()
        .map(|(_, pos)| match neighbourhood.section(pos) {
            Some(cells) => {
                let ids: Vec<VoxelId> = cells.iter().map(|&id| VoxelId(id)).collect();
                SectionSource::Loaded(Arc::new(VoxelPalette::from_cells(&ids)))
            }
            None if neighbourhood
                .bounds
                .is_outside(pos.y * SectionPos::SIZE as i32) =>
            {
                SectionSource::Open(registry.outside())
            }
            None => SectionSource::Absent(registry.unloaded()),
        })
        .collect();
    let blocks = BlockSnapshot::new(&layout, sources);
    let field = LightField::new(layout.clone());

    let mut seeds = Vec::new();
    for (section, _) in layout.sections() {
        let base = layout.section_base(section);
        for local in LocalPos::all() {
            let index = base | local.index() as u32;
            let id = blocks.get(index);
            let emission = registry.emission(id);
            if !emission.is_zero() && colours.light_type(id) == t {
                field.set(index, emission);
                seeds.push(index);
            }
        }
    }
    relax(&field, &blocks, &registry, seeds);

    let section_index = |pos: SectionPos| {
        layout
            .sections()
            .find(|&(_, p)| p == pos)
            .map(|(i, _)| i)
            .expect("an output cell outside the oracle layout")
    };
    output_positions(centre)
        .map(|pos| {
            let base = layout.section_base(section_index(SectionPos::from(pos)));
            field.get(base | LocalPos::from(pos).index() as u32).get()
        })
        .collect()
}

pub struct Computed {
    pub region: Region,
    pub palette: Palette,
    pub lanes: Lanes,
}

pub fn compute(neighbourhood: &Neighbourhood) -> Computed {
    compute_with(neighbourhood, None)
}

/// Runs the colour propagation, optionally restricted to a smaller palette
/// than the region's own.
pub fn compute_with(neighbourhood: &Neighbourhood, palette: Option<Palette>) -> Computed {
    let registry = registry();
    let colours = colours();
    let (min, size) = section_output(neighbourhood.centre);
    let region = Region::new(
        min,
        size,
        neighbourhood.bounds,
        &registry,
        neighbourhood.cells(),
    );
    let palette = palette.unwrap_or_else(|| Palette::of(&region, &registry, &colours));
    let costs = EdgeCosts::new(&region, &registry);
    let lanes = propagate(&region, &costs, &palette, &registry, &colours);
    Computed {
        region,
        palette,
        lanes,
    }
}

/// The first output cell where a lane differs from the oracle.
pub fn lane_mismatch(
    neighbourhood: &Neighbourhood,
    computed: &Computed,
    t: LightType,
) -> Option<String> {
    let lane = computed.palette.lane(t)?;
    let expected = oracle(neighbourhood, t);
    let region = &computed.region;
    output_positions(neighbourhood.centre)
        .zip(expected)
        .find_map(|(pos, want)| {
            let cell = region.index(
                pos.x - region.min.x,
                pos.y - region.min.y,
                pos.z - region.min.z,
            );
            let got = computed.lanes.level(cell, lane);
            (got != want).then(|| format!("type {} at {pos}: colour lane {got}, relax {want}", t.0))
        })
}

pub fn assert_lanes_match(neighbourhood: &Neighbourhood) -> Computed {
    let computed = compute(neighbourhood);
    for &t in &computed.palette.types {
        if let Some(mismatch) = lane_mismatch(neighbourhood, &computed, t) {
            panic!("{mismatch}");
        }
    }
    computed
}
