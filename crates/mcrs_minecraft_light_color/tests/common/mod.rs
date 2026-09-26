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
use mcrs_minecraft_light_color::region::{EdgeCosts, Palette, Region, section_output};

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
