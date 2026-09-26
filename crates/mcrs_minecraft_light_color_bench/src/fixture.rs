use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy_math::{IVec3, Vec3};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use mcrs_minecraft_chunk::{VoxelId, VoxelPalette};
use mcrs_minecraft_core::voxel_shape::{Aabb, ShapeRegistry, VoxelShape};
use mcrs_minecraft_core::{BlockPos, BoundingBox, LocalPos, SectionPos};
use mcrs_minecraft_light::block::{LightProperties, LightRegistry, SpecialBlocks};
use mcrs_minecraft_light::field::{BlockSnapshot, FieldLayout, LightField, SectionSource};
use mcrs_minecraft_light::level::{LightBounds, LightLevel};
use mcrs_minecraft_light::relax;
use mcrs_minecraft_light_color::asset::BlockStateRef;
use mcrs_minecraft_light_color::colors::{LightColors, LightType};
use mcrs_minecraft_light_color::region::section_output;
use serde::{Deserialize, Serialize};

/// Sections per side of a fixture: the 3×3×3 block the report measures plus
/// the ring every inner section's region reaches into.
pub const SIDE: i32 = 5;
pub const SECTIONS: usize = (SIDE * SIDE * SIDE) as usize;
const VOLUME: usize = SectionPos::VOLUME;

/// Sections run x fastest, then z, then y, from `origin`; `None` is a section
/// the save lacks or one outside the world. A palette index is the block id.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub dimension: String,
    pub min_section_y: i32,
    pub section_count: u32,
    pub origin: [i32; 3],
    pub colours: Vec<[u8; 3]>,
    pub palette: Vec<FixtureState>,
    pub sections: Vec<Option<FixtureSection>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureState {
    pub state: BlockStateRef,
    pub dampening: u8,
    pub emission: u8,
    pub occlusion: Option<Vec<[[f32; 3]; 2]>>,
    pub light_type: u8,
}

/// Cells in `y << 8 | z << 4 | x` order; block light as the save packs it,
/// two cells per byte, low nibble first.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureSection {
    pub blocks: Vec<u16>,
    pub block_light: Option<Vec<u8>>,
}

impl Fixture {
    pub fn read(path: &Path) -> Fixture {
        let file = File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let fixture: Fixture = serde_json::from_reader(BufReader::new(GzDecoder::new(file)))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        if let Err(problem) = fixture.check() {
            panic!("{}: {problem}", path.display());
        }
        fixture
    }

    pub fn write(&self, path: &Path) {
        self.check().unwrap_or_else(|problem| panic!("{problem}"));
        let file = File::create(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut encoder = GzEncoder::new(BufWriter::new(file), Compression::best());
        serde_json::to_writer(&mut encoder, self).expect("a fixture serializes");
        encoder
            .finish()
            .and_then(|mut w| w.flush())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }

    fn check(&self) -> Result<(), String> {
        if self.sections.len() != SECTIONS {
            return Err(format!(
                "{} sections, expected {SECTIONS}",
                self.sections.len()
            ));
        }
        if self.palette.len() + 2 > u16::MAX as usize {
            return Err(format!(
                "{} palette entries leave no room for the fillers",
                self.palette.len()
            ));
        }
        for (i, state) in self.palette.iter().enumerate() {
            if state.dampening > 15 || state.emission > 15 {
                return Err(format!(
                    "palette entry {i} `{}` is past level 15",
                    state.state
                ));
            }
            if state.light_type as usize > self.colours.len() {
                return Err(format!(
                    "palette entry {i} `{}` names a type past the colours",
                    state.state
                ));
            }
        }
        for (i, section) in self.sections.iter().enumerate() {
            let Some(section) = section else { continue };
            if section.blocks.len() != VOLUME {
                return Err(format!("section {i} has {} cells", section.blocks.len()));
            }
            if let Some(&id) = section
                .blocks
                .iter()
                .find(|&&id| id as usize >= self.palette.len())
            {
                return Err(format!("section {i} names block {id}, past the palette"));
            }
            if let Some(light) = &section.block_light
                && light.len() != VOLUME / 2
            {
                return Err(format!("section {i} has {} block light bytes", light.len()));
            }
        }
        Ok(())
    }

    /// Builds the light table the same way the server's does for the corpus:
    /// one row per state, then the unloaded and outside fillers.
    pub fn scene(&self, name: &str) -> Scene {
        let mut shapes = ShapeRegistry::new();
        let mut properties: Vec<LightProperties> = self
            .palette
            .iter()
            .map(|state| LightProperties {
                dampening: state.dampening,
                emission: LightLevel::new(state.emission),
                occlusion: state.occlusion.as_ref().map(|boxes| {
                    let boxes: Vec<Aabb> = boxes
                        .iter()
                        .map(|[min, max]| Aabb {
                            min: Vec3::from_array(*min),
                            max: Vec3::from_array(*max),
                        })
                        .collect();
                    shapes.intern(VoxelShape::from_boxes(&boxes))
                }),
            })
            .collect();
        let unloaded = VoxelId(properties.len() as u16);
        properties.push(LightProperties::SOLID);
        properties.push(LightProperties::AIR);
        let registry = LightRegistry::new(
            properties,
            SpecialBlocks {
                unloaded,
                outside: VoxelId(unloaded.0 + 1),
            },
        );
        let types: Vec<LightType> = self
            .palette
            .iter()
            .map(|state| LightType(state.light_type))
            .collect();

        let (blocks, light) = self
            .sections
            .iter()
            .map(|section| match section {
                None => (None, None),
                Some(section) => {
                    let blocks: Box<[u16; VOLUME]> = section
                        .blocks
                        .clone()
                        .into_boxed_slice()
                        .try_into()
                        .unwrap();
                    let light = section.block_light.as_ref().map(|packed| {
                        Box::new(std::array::from_fn(|i| {
                            (packed[i >> 1] >> ((i & 1) * 4)) & 15
                        }))
                    });
                    (Some(blocks), light)
                }
            })
            .unzip();

        Scene {
            name: name.to_owned(),
            bounds: LightBounds::new(
                self.min_section_y,
                self.min_section_y + self.section_count as i32 - 1,
            ),
            origin: SectionPos::new(self.origin[0], self.origin[1], self.origin[2]),
            registry,
            colours: LightColors::new(self.colours.clone(), types),
            blocks,
            light,
        }
    }
}

pub struct Scene {
    pub name: String,
    pub bounds: LightBounds,
    pub origin: SectionPos,
    pub registry: LightRegistry,
    pub colours: LightColors,
    pub blocks: Vec<Option<Box<[u16; VOLUME]>>>,
    pub light: Vec<Option<Box<[u8; VOLUME]>>>,
}

impl Scene {
    pub fn slot(&self, pos: SectionPos) -> Option<usize> {
        let d = pos.0 - self.origin.0;
        (d.cmpge(IVec3::ZERO).all() && d.cmplt(IVec3::splat(SIDE)).all())
            .then(|| (d.x + SIDE * (d.z + SIDE * d.y)) as usize)
    }

    pub fn section_at(&self, slot: usize) -> SectionPos {
        let slot = slot as i32;
        SectionPos::new(
            self.origin.x + slot % SIDE,
            self.origin.y + slot / (SIDE * SIDE),
            self.origin.z + slot / SIDE % SIDE,
        )
    }

    pub fn section(&self, pos: SectionPos) -> Option<&[u16; VOLUME]> {
        self.slot(pos).and_then(|i| self.blocks[i].as_deref())
    }

    pub fn cells<'a>(&'a self) -> impl Fn(SectionPos) -> Option<&'a [u16; VOLUME]> + 'a {
        |pos| self.section(pos)
    }

    /// The 3×3×3 sections whose regions stay inside the fixture.
    pub fn inner(&self) -> impl Iterator<Item = SectionPos> + '_ {
        (1..=3).flat_map(move |dy| {
            (1..=3).flat_map(move |dz| {
                (1..=3).map(move |dx| {
                    SectionPos::new(self.origin.x + dx, self.origin.y + dy, self.origin.z + dz)
                })
            })
        })
    }

    pub fn inner_min(&self) -> BlockPos {
        let s = SectionPos::SIZE as i32;
        BlockPos::new(
            (self.origin.x + 1) * s,
            (self.origin.y + 1) * s,
            (self.origin.z + 1) * s,
        )
    }

    pub fn server_level(&self, pos: BlockPos) -> u8 {
        self.slot(SectionPos::from(pos))
            .and_then(|i| self.light[i].as_deref())
            .map_or(0, |light| light[LocalPos::from(pos).index()])
    }
}

pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

pub fn scene(name: &str) -> Scene {
    Fixture::read(&fixtures_dir().join(format!("{name}.json.gz"))).scene(name)
}

pub fn scenes() -> Vec<Scene> {
    let mut names: Vec<String> = std::fs::read_dir(fixtures_dir())
        .expect("the fixtures directory exists")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".json.gz").map(str::to_owned)
        })
        .collect();
    names.sort();
    names.iter().map(|name| scene(name)).collect()
}

/// Output cells of a section, x fastest then z then y.
pub fn output_positions(section: SectionPos) -> impl Iterator<Item = BlockPos> {
    let (min, size) = section_output(section);
    (0..size).flat_map(move |y| {
        (0..size).flat_map(move |z| {
            (0..size).map(move |x| BlockPos::new(min.x + x, min.y + y, min.z + z))
        })
    })
}

/// Light of type `t` on the section's output, from the server's relax seeded
/// with that type's emitters only.
pub fn oracle(scene: &Scene, section: SectionPos, t: LightType) -> Vec<u8> {
    let area = BoundingBox::of_section(section).inflated(SectionPos::SIZE as i32);
    let (layout, field) = relaxed(scene, area, |seed| seed == t);
    output_positions(section)
        .map(|pos| field.get(cell(&layout, pos)).get())
        .collect()
}

/// Block light of every emitter together over the whole fixture, per slot.
pub fn relaxed_block_light(scene: &Scene) -> Vec<Option<Box<[u8; VOLUME]>>> {
    let centre = SectionPos(scene.origin.0 + IVec3::splat(SIDE / 2));
    let area = BoundingBox::of_section(centre).inflated(SIDE / 2 * SectionPos::SIZE as i32);
    let (layout, field) = relaxed(scene, area, |_| true);
    (0..SECTIONS)
        .map(|slot| {
            scene.blocks[slot].as_ref()?;
            let base = BlockPos::from(scene.section_at(slot).0 * SectionPos::SIZE as i32);
            Some(Box::new(std::array::from_fn(|i| {
                let local = LocalPos::from_index(i);
                let pos = BlockPos::new(
                    base.x + local.x() as i32,
                    base.y + local.y() as i32,
                    base.z + local.z() as i32,
                );
                field.get(cell(&layout, pos)).get()
            })))
        })
        .collect()
}

fn cell(layout: &FieldLayout, pos: BlockPos) -> u32 {
    let section = SectionPos::from(pos);
    let (index, _) = layout
        .sections()
        .find(|&(_, p)| p == section)
        .expect("a cell outside the relaxed layout");
    layout.section_base(index) | LocalPos::from(pos).index() as u32
}

fn relaxed(
    scene: &Scene,
    area: BoundingBox,
    keep: impl Fn(LightType) -> bool,
) -> (FieldLayout, LightField) {
    let registry = &scene.registry;
    let layout = FieldLayout::covering(area);
    let sources = layout
        .sections()
        .map(|(_, pos)| match scene.section(pos) {
            Some(cells) => {
                let ids: Vec<VoxelId> = cells.iter().map(|&id| VoxelId(id)).collect();
                SectionSource::Loaded(Arc::new(VoxelPalette::from_cells(&ids)))
            }
            None if scene.bounds.is_outside(pos.y * SectionPos::SIZE as i32) => {
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
            if !emission.is_zero() && keep(scene.colours.light_type(id)) {
                field.set(index, emission);
                seeds.push(index);
            }
        }
    }
    relax(&field, &blocks, registry, seeds);
    (layout, field)
}
