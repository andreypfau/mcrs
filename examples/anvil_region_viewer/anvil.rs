//! Reads an Anvil region file (`r.X.Z.mca`) into flat, mesher-friendly section arrays.
//!
//! The region is loaded once and never edited, so every block state in the file is interned into a
//! single global table and each section is expanded to a dense `u16` index array. That trades ~8 KB
//! per non-empty section for random neighbour access with no palette indirection in the mesher.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;

use serde::Deserialize;

pub const SECTION_SIZE: usize = 16;
pub const SECTION_VOLUME: usize = SECTION_SIZE * SECTION_SIZE * SECTION_SIZE;
/// Chunks along one axis of a region file.
pub const REGION_CHUNKS: usize = 32;

/// Blocks along one edge of a region file.
pub const REGION_BLOCKS: usize = REGION_CHUNKS * SECTION_SIZE;

/// A block state as it appears in a section palette: `minecraft:oak_log` plus its properties,
/// sorted so two palettes that list the same properties in a different order intern to one id.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct BlockStateKey {
    pub name: String,
    pub props: Vec<(String, String)>,
}

impl BlockStateKey {
    pub fn pairs(&self) -> Vec<(&str, &str)> {
        let mut out = Vec::with_capacity(self.props.len());
        for (k, v) in &self.props {
            out.push((k.as_str(), v.as_str()));
        }
        out
    }

    pub fn label(&self) -> String {
        if self.props.is_empty() {
            return self.name.clone();
        }
        let mut s = String::new();
        for (i, (k, v)) in self.props.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(k);
            s.push('=');
            s.push_str(v);
        }
        format!("{}[{}]", self.name, s)
    }
}

/// One 16³ section, expanded out of its palette. `blocks` is indexed `y * 256 + z * 16 + x`, the
/// order the file itself uses, and holds indices into [`Region::states`].
pub struct Section {
    pub blocks: Box<[u16; SECTION_VOLUME]>,
    /// Biome index per 4³ cell, indexed `y * 16 + z * 4 + x`, into [`Region::biomes`].
    pub biomes: Box<[u8; 64]>,
}

pub struct Region {
    pub states: Vec<BlockStateKey>,
    pub biomes: Vec<String>,
    pub air_state: u16,
    pub min_section_y: i32,
    pub sections_y: usize,
    /// `(cz * REGION_CHUNKS + cx) * sections_y + (sy - min_section_y)`; `None` for a section that is
    /// absent from the file or uniformly air.
    sections: Vec<Option<Section>>,
    /// `block_light << 4 | sky_light`, indexed like `sections`. Kept apart from them because a
    /// section the mesher drops as uniformly air still lights the faces around it.
    lights: Vec<Option<Box<[u8; SECTION_VOLUME]>>>,
    /// Per chunk column, the first section index above every stored light array. The file stops
    /// writing sky light once nothing shadows it, so from here up the column is open sky. Zero for
    /// a column the file never lit, which then reads as open sky throughout.
    light_top: Vec<u16>,
}

impl Region {
    #[inline]
    pub fn section(&self, cx: usize, sy: usize, cz: usize) -> Option<&Section> {
        self.sections[(cz * REGION_CHUNKS + cx) * self.sections_y + sy].as_ref()
    }

    pub fn non_empty_sections(&self) -> usize {
        let mut n = 0;
        for s in &self.sections {
            if s.is_some() {
                n += 1;
            }
        }
        n
    }

    /// Block state id at a region-local block coordinate, or `air_state` outside the region.
    #[inline]
    pub fn block(&self, x: i32, y: i32, z: i32) -> u16 {
        let sy = y.div_euclid(SECTION_SIZE as i32) - self.min_section_y;
        if x < 0
            || z < 0
            || x >= (REGION_CHUNKS * SECTION_SIZE) as i32
            || z >= (REGION_CHUNKS * SECTION_SIZE) as i32
            || sy < 0
            || sy as usize >= self.sections_y
        {
            return self.air_state;
        }
        let cx = x as usize / SECTION_SIZE;
        let cz = z as usize / SECTION_SIZE;
        match self.section(cx, sy as usize, cz) {
            Some(section) => {
                let lx = x as usize % SECTION_SIZE;
                let lz = z as usize % SECTION_SIZE;
                let ly = y.rem_euclid(SECTION_SIZE as i32) as usize;
                section.blocks[ly * 256 + lz * SECTION_SIZE + lx]
            }
            None => self.air_state,
        }
    }

    /// `block_light << 4 | sky_light` at a region-local block coordinate.
    ///
    /// The file omits a nibble array that is entirely zero, so a stored section without one is
    /// dark, not lit; only above the column's topmost stored array does full sky take over.
    #[inline]
    pub fn light(&self, x: i32, y: i32, z: i32) -> u8 {
        let sy = y.div_euclid(SECTION_SIZE as i32) - self.min_section_y;
        if x < 0
            || z < 0
            || x >= (REGION_CHUNKS * SECTION_SIZE) as i32
            || z >= (REGION_CHUNKS * SECTION_SIZE) as i32
        {
            return 0x0f;
        }
        if sy < 0 {
            return 0x00;
        }
        let cx = x as usize / SECTION_SIZE;
        let cz = z as usize / SECTION_SIZE;
        let column = cz * REGION_CHUNKS + cx;
        if sy as usize >= self.light_top[column] as usize {
            return 0x0f;
        }
        match self.lights[column * self.sections_y + sy as usize].as_ref() {
            Some(light) => {
                let lx = x as usize % SECTION_SIZE;
                let lz = z as usize % SECTION_SIZE;
                let ly = y.rem_euclid(SECTION_SIZE as i32) as usize;
                light[ly * 256 + lz * SECTION_SIZE + lx]
            }
            None => 0x00,
        }
    }
}

/// A rectangle of region files addressed as one coordinate space.
///
/// Each file interns its own block states and biomes, so a region joining the world is given a
/// table remapping its ids onto the world's shared ones and reads go through that. Rewriting the
/// section arrays instead would cost a pass over every block in the region for nothing: the remap
/// is a few hundred entries and stays in cache.
///
/// Horizontal coordinates are relative to the window's own corner and the vertical one is the
/// world's, which is the same split [`Region`] itself uses.
pub struct World {
    pub states: Vec<BlockStateKey>,
    pub biomes: Vec<String>,
    pub air_state: u16,
    /// Section coordinates of the window's corner, in the world's own signed numbering. Region
    /// coordinates run either side of zero, so none of the three may be assumed non-negative.
    pub min_section: [i32; 3],
    /// The window's extent in sections.
    pub sections: [usize; 3],
    /// Region coordinates of the window's corner and its extent in region files.
    pub min_region: [i32; 2],
    pub regions: [usize; 2],
    /// Row-major over the window, `rz * regions[0] + rx`.
    slots: Vec<Option<Resident>>,
    intern: HashMap<BlockStateKey, u16>,
    biome_intern: HashMap<String, u8>,
}

/// One loaded region and the tables mapping its own ids onto the world's.
struct Resident {
    region: Region,
    states: Vec<u16>,
    biomes: Vec<u8>,
}

impl World {
    /// An empty window of `regions` region files with its corner at `min_region`.
    pub fn new(min_region: [i32; 2], regions: [usize; 2]) -> Self {
        let mut states = Vec::new();
        let mut intern = HashMap::new();
        let air_state = intern_state(
            &mut intern,
            &mut states,
            BlockStateKey {
                name: "minecraft:air".to_string(),
                props: Vec::new(),
            },
        );
        Self {
            states,
            biomes: Vec::new(),
            air_state,
            min_section: [min_region[0] * REGION_CHUNKS as i32, 0, min_region[1] * REGION_CHUNKS as i32],
            sections: [regions[0] * REGION_CHUNKS, 0, regions[1] * REGION_CHUNKS],
            min_region,
            regions,
            slots: (0..regions[0] * regions[1]).map(|_| None).collect(),
            intern,
            biome_intern: HashMap::new(),
        }
    }

    /// Puts a parsed region into the window, remapping its ids onto the world's shared tables and
    /// widening the world's vertical span to cover it.
    pub fn insert(&mut self, coords: [i32; 2], region: Region) {
        let Some(slot) = self.slot_of(coords) else {
            return;
        };
        let states = region
            .states
            .iter()
            .map(|key| intern_state(&mut self.intern, &mut self.states, key.clone()))
            .collect();
        let biomes = region
            .biomes
            .iter()
            .map(|name| intern_biome(&mut self.biome_intern, &mut self.biomes, name))
            .collect();

        let low = region.min_section_y;
        let high = low + region.sections_y as i32;
        let (world_low, world_high) = match self.sections[1] {
            0 => (low, high),
            span => (
                self.min_section[1].min(low),
                (self.min_section[1] + span as i32).max(high),
            ),
        };
        self.min_section[1] = world_low;
        self.sections[1] = (world_high - world_low) as usize;

        self.slots[slot] = Some(Resident {
            region,
            states,
            biomes,
        });
    }

    fn slot_of(&self, coords: [i32; 2]) -> Option<usize> {
        let rx = coords[0] - self.min_region[0];
        let rz = coords[1] - self.min_region[1];
        if rx < 0 || rz < 0 || rx as usize >= self.regions[0] || rz as usize >= self.regions[1] {
            return None;
        }
        Some(rz as usize * self.regions[0] + rx as usize)
    }

    /// The region covering a window-relative block column, if it is loaded. `div_euclid` rather
    /// than a plain division: the mesher reads one block outside the section it is meshing, which
    /// at the window's corner is a negative coordinate that truncating division would round the
    /// wrong way and land back inside the first region.
    #[inline]
    fn resident(&self, x: i32, z: i32) -> Option<&Resident> {
        let span = REGION_BLOCKS as i32;
        let rx = x.div_euclid(span);
        let rz = z.div_euclid(span);
        if rx < 0 || rz < 0 || rx as usize >= self.regions[0] || rz as usize >= self.regions[1] {
            return None;
        }
        self.slots[rz as usize * self.regions[0] + rx as usize].as_ref()
    }

    /// Block state id at a window-relative horizontal position and a world vertical one, in the
    /// world's shared numbering. Air wherever no region is loaded.
    #[inline]
    pub fn block(&self, x: i32, y: i32, z: i32) -> u16 {
        let span = REGION_BLOCKS as i32;
        match self.resident(x, z) {
            Some(resident) => {
                let local = resident
                    .region
                    .block(x.rem_euclid(span), y, z.rem_euclid(span));
                resident.states[local as usize]
            }
            None => self.air_state,
        }
    }

    /// `block_light << 4 | sky_light`. Outside the loaded window this is open sky, which is what
    /// keeps the window's outer wall lit rather than ringed in black.
    #[inline]
    pub fn light(&self, x: i32, y: i32, z: i32) -> u8 {
        let span = REGION_BLOCKS as i32;
        match self.resident(x, z) {
            Some(resident) => resident
                .region
                .light(x.rem_euclid(span), y, z.rem_euclid(span)),
            None => 0x0f,
        }
    }

    /// One section by window-relative section coordinates, all three of them.
    #[inline]
    pub fn section(&self, sx: usize, sy: usize, sz: usize) -> Option<&Section> {
        let chunks = REGION_CHUNKS;
        let resident = self.slots[sz / chunks * self.regions[0] + sx / chunks].as_ref()?;
        let local_y = self.min_section[1] + sy as i32 - resident.region.min_section_y;
        if local_y < 0 || local_y as usize >= resident.region.sections_y {
            return None;
        }
        resident
            .region
            .section(sx % chunks, local_y as usize, sz % chunks)
    }

    /// The biome of a section cell, in the world's shared numbering.
    pub fn biome(&self, sx: usize, sy: usize, sz: usize, cell: usize) -> u8 {
        let chunks = REGION_CHUNKS;
        let Some(resident) = self.slots[sz / chunks * self.regions[0] + sx / chunks].as_ref() else {
            return 0;
        };
        match self.section(sx, sy, sz) {
            Some(section) => resident.biomes[section.biomes[cell] as usize],
            None => 0,
        }
    }

    pub fn loaded(&self) -> usize {
        self.slots.iter().filter(|slot| slot.is_some()).count()
    }

    pub fn non_empty_sections(&self) -> usize {
        self.slots
            .iter()
            .flatten()
            .map(|resident| resident.region.non_empty_sections())
            .sum()
    }
}

#[derive(Deserialize)]
struct ChunkNbt {
    #[serde(rename = "yPos", default)]
    y_pos: i32,
    #[serde(default)]
    sections: Vec<SectionNbt>,
}

#[derive(Deserialize)]
struct SectionNbt {
    #[serde(rename = "Y")]
    y: i8,
    #[serde(default)]
    block_states: Option<BlockStatesNbt>,
    #[serde(default)]
    biomes: Option<BiomesNbt>,
    #[serde(rename = "BlockLight", default)]
    block_light: Option<Vec<i8>>,
    #[serde(rename = "SkyLight", default)]
    sky_light: Option<Vec<i8>>,
}

#[derive(Deserialize)]
struct BlockStatesNbt {
    #[serde(default)]
    palette: Vec<PaletteEntryNbt>,
    #[serde(default)]
    data: Option<Vec<i64>>,
}

#[derive(Deserialize)]
struct BiomesNbt {
    #[serde(default)]
    palette: Vec<String>,
    #[serde(default)]
    data: Option<Vec<i64>>,
}

#[derive(Deserialize)]
struct PaletteEntryNbt {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Properties", default)]
    properties: HashMap<String, String>,
}

/// The region coordinates a region file's name carries, or `None` for anything else.
pub fn region_coords(name: &str) -> Option<[i32; 2]> {
    let (x, z) = name.strip_prefix("r.")?.strip_suffix(".mca")?.split_once('.')?;
    Some([x.parse().ok()?, z.parse().ok()?])
}

/// Loads a window of `size` by `size` region files centred on `centre`, or the one file named.
///
/// The names of the wanted files are built rather than searched for. `poi/` and `entities/` sit
/// beside `region/` holding files named `r.X.Z.mca` too, of a completely different shape, and any
/// search that walked into them would fail on the first one it tried to parse.
pub fn load_world(path: &Path, centre: [i32; 2], size: usize) -> Result<World, String> {
    if !path.is_dir() {
        let coords = path
            .file_name()
            .and_then(|name| region_coords(&name.to_string_lossy()))
            .unwrap_or([0, 0]);
        let mut world = World::new(coords, [1, 1]);
        world.insert(coords, load(path)?);
        return Ok(world);
    }

    // Half the window below the centre, so a window of any size holds the centre region and an
    // even one straddles it rather than sitting entirely on its positive side.
    let min = [
        centre[0] - (size / 2) as i32,
        centre[1] - (size / 2) as i32,
    ];
    let mut world = World::new(min, [size, size]);
    for rz in 0..size as i32 {
        for rx in 0..size as i32 {
            let coords = [min[0] + rx, min[1] + rz];
            let file = path.join(format!("r.{}.{}.mca", coords[0], coords[1]));
            if !file.is_file() {
                continue;
            }
            world.insert(coords, load(&file)?);
        }
    }
    if world.loaded() == 0 {
        return Err(format!(
            "{} holds none of the {size}x{size} region files around r.{}.{}",
            path.display(),
            centre[0],
            centre[1],
        ));
    }
    Ok(world)
}

pub fn load(path: &Path) -> Result<Region, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if bytes.len() < 8192 {
        return Err(format!("{} is shorter than a region header", path.display()));
    }

    let mut intern: HashMap<BlockStateKey, u16> = HashMap::new();
    let mut states: Vec<BlockStateKey> = Vec::new();
    let mut biome_intern: HashMap<String, u8> = HashMap::new();
    let mut biomes: Vec<String> = Vec::new();
    let air_state = intern_state(
        &mut intern,
        &mut states,
        BlockStateKey {
            name: "minecraft:air".to_string(),
            props: Vec::new(),
        },
    );

    let mut chunks: Vec<(usize, ChunkNbt)> = Vec::new();
    let mut scratch = Vec::new();
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;

    for slot in 0..REGION_CHUNKS * REGION_CHUNKS {
        let head = slot * 4;
        let offset = u32::from_be_bytes([0, bytes[head], bytes[head + 1], bytes[head + 2]]) as usize;
        let sector_count = bytes[head + 3] as usize;
        if offset == 0 || sector_count == 0 {
            continue;
        }
        let start = offset * 4096;
        if start + 5 > bytes.len() {
            return Err(format!("chunk {slot} points past the end of the file"));
        }
        let length = u32::from_be_bytes([
            bytes[start],
            bytes[start + 1],
            bytes[start + 2],
            bytes[start + 3],
        ]) as usize;
        let scheme = bytes[start + 4];
        let payload_end = start + 4 + length;
        if length == 0 || payload_end > bytes.len() {
            return Err(format!("chunk {slot} declares an impossible payload length"));
        }
        let payload = &bytes[start + 5..payload_end];

        scratch.clear();
        let nbt: &[u8] = match scheme {
            1 => {
                flate2::read::GzDecoder::new(payload)
                    .read_to_end(&mut scratch)
                    .map_err(|e| format!("chunk {slot}: gzip: {e}"))?;
                &scratch
            }
            2 => {
                flate2::read::ZlibDecoder::new(payload)
                    .read_to_end(&mut scratch)
                    .map_err(|e| format!("chunk {slot}: zlib: {e}"))?;
                &scratch
            }
            3 => payload,
            other => return Err(format!("chunk {slot}: unsupported compression {other}")),
        };

        let chunk: ChunkNbt = mcrs_nbt::from_bytes(Cursor::new(nbt))
            .map_err(|e| format!("chunk {slot}: cannot deserialize: {e}"))?;
        for section in &chunk.sections {
            min_y = min_y.min(section.y as i32);
            max_y = max_y.max(section.y as i32);
        }
        let _ = chunk.y_pos;
        chunks.push((slot, chunk));
    }

    if chunks.is_empty() {
        return Err(format!("{} contains no chunks", path.display()));
    }

    let min_section_y = min_y;
    let sections_y = (max_y - min_y + 1) as usize;
    let slots = REGION_CHUNKS * REGION_CHUNKS * sections_y;
    let mut sections: Vec<Option<Section>> = Vec::with_capacity(slots);
    sections.resize_with(slots, || None);
    let mut lights: Vec<Option<Box<[u8; SECTION_VOLUME]>>> = Vec::with_capacity(slots);
    lights.resize_with(slots, || None);

    for (slot, chunk) in chunks {
        let cx = slot % REGION_CHUNKS;
        let cz = slot / REGION_CHUNKS;
        for section in chunk.sections {
            let sy = section.y as i32 - min_section_y;
            if sy < 0 || sy as usize >= sections_y {
                continue;
            }
            let slot = (cz * REGION_CHUNKS + cx) * sections_y + sy as usize;
            lights[slot] = unpack_light(&section);
            let Some(built) = build_section(
                &section,
                &mut intern,
                &mut states,
                &mut biome_intern,
                &mut biomes,
                air_state,
            )?
            else {
                continue;
            };
            sections[slot] = Some(built);
        }
    }

    let mut light_top = vec![0u16; REGION_CHUNKS * REGION_CHUNKS];
    for (column, top) in light_top.iter_mut().enumerate() {
        let base = column * sections_y;
        for sy in (0..sections_y).rev() {
            if lights[base + sy].is_some() {
                *top = sy as u16 + 1;
                break;
            }
        }
    }

    Ok(Region {
        states,
        biomes,
        air_state,
        min_section_y,
        sections_y,
        sections,
        lights,
        light_top,
    })
}

fn intern_state(
    intern: &mut HashMap<BlockStateKey, u16>,
    states: &mut Vec<BlockStateKey>,
    key: BlockStateKey,
) -> u16 {
    if let Some(&id) = intern.get(&key) {
        return id;
    }
    let id = states.len() as u16;
    states.push(key.clone());
    intern.insert(key, id);
    id
}

fn intern_biome(
    intern: &mut HashMap<String, u8>,
    names: &mut Vec<String>,
    name: &str,
) -> u8 {
    if let Some(&id) = intern.get(name) {
        return id;
    }
    let id = names.len() as u8;
    names.push(name.to_string());
    intern.insert(name.to_string(), id);
    id
}

fn build_section(
    section: &SectionNbt,
    intern: &mut HashMap<BlockStateKey, u16>,
    states: &mut Vec<BlockStateKey>,
    biome_intern: &mut HashMap<String, u8>,
    biome_names: &mut Vec<String>,
    air_state: u16,
) -> Result<Option<Section>, String> {
    let Some(block_states) = section.block_states.as_ref() else {
        return Ok(None);
    };
    if block_states.palette.is_empty() {
        return Ok(None);
    }

    let mut palette: Vec<u16> = Vec::with_capacity(block_states.palette.len());
    for entry in &block_states.palette {
        let mut props: Vec<(String, String)> = Vec::with_capacity(entry.properties.len());
        for (k, v) in &entry.properties {
            props.push((k.clone(), v.clone()));
        }
        props.sort_unstable();
        palette.push(intern_state(
            intern,
            states,
            BlockStateKey {
                name: entry.name.clone(),
                props,
            },
        ));
    }

    // A palette of one and no data array means the whole section is that single state.
    let Some(data) = block_states.data.as_ref() else {
        if palette[0] == air_state {
            return Ok(None);
        }
        return Ok(Some(Section {
            blocks: Box::new([palette[0]; SECTION_VOLUME]),
            biomes: unpack_biomes(section, biome_intern, biome_names),
        }));
    };

    let bits = bits_per_entry(palette.len());
    let per_long = 64 / bits;
    let needed = SECTION_VOLUME.div_ceil(per_long);
    if data.len() < needed {
        return Err(format!(
            "section holds {} longs but a {bits}-bit palette of {} needs {needed}",
            data.len(),
            palette.len()
        ));
    }

    let mask = (1u64 << bits) - 1;
    let mut blocks = Box::new([air_state; SECTION_VOLUME]);
    let mut written = 0usize;
    let mut solid = false;
    'outer: for &word in data.iter() {
        let word = word as u64;
        for slot in 0..per_long {
            let index = ((word >> (slot * bits)) & mask) as usize;
            if index >= palette.len() {
                return Err(format!(
                    "palette index {index} out of range for a palette of {}",
                    palette.len()
                ));
            }
            let state = palette[index];
            blocks[written] = state;
            solid |= state != air_state;
            written += 1;
            if written == SECTION_VOLUME {
                break 'outer;
            }
        }
    }
    if !solid {
        return Ok(None);
    }
    Ok(Some(Section {
        blocks,
        biomes: unpack_biomes(section, biome_intern, biome_names),
    }))
}

/// Biome cells are 4³, so a section holds 64 of them indexed `y * 16 + z * 4 + x`. Unlike the block
/// palette this container has no 4-bit floor: a two-entry palette really is packed one bit wide.
fn unpack_biomes(
    section: &SectionNbt,
    intern: &mut HashMap<String, u8>,
    names: &mut Vec<String>,
) -> Box<[u8; 64]> {
    let mut out = Box::new([0u8; 64]);
    let Some(source) = section.biomes.as_ref() else {
        return out;
    };
    if source.palette.is_empty() {
        return out;
    }
    let mut palette: Vec<u8> = Vec::with_capacity(source.palette.len());
    for name in &source.palette {
        palette.push(match intern.get(name) {
            Some(&id) => id,
            None => {
                // More than 256 biomes in one region cannot happen; clamp rather than panic.
                let id = names.len().min(u8::MAX as usize) as u8;
                names.push(name.clone());
                intern.insert(name.clone(), id);
                id
            }
        });
    }
    let Some(data) = source.data.as_ref() else {
        out.fill(palette[0]);
        return out;
    };
    let bits = (usize::BITS - source.palette.len().saturating_sub(1).leading_zeros()).max(1) as usize;
    let per_long = 64 / bits;
    let mask = (1u64 << bits) - 1;
    let mut written = 0usize;
    'outer: for &word in data.iter() {
        let word = word as u64;
        for slot in 0..per_long {
            let index = ((word >> (slot * bits)) & mask) as usize;
            out[written] = palette[index.min(palette.len() - 1)];
            written += 1;
            if written == 64 {
                break 'outer;
            }
        }
    }
    out
}

/// Post-1.16 packing: entries never straddle a `long`, so a `long` holds `64 / bits` of them.
#[inline]
fn bits_per_entry(palette_len: usize) -> usize {
    let needed = usize::BITS - (palette_len.saturating_sub(1)).leading_zeros();
    (needed as usize).max(4)
}

fn unpack_light(section: &SectionNbt) -> Option<Box<[u8; SECTION_VOLUME]>> {
    if section.block_light.is_none() && section.sky_light.is_none() {
        return None;
    }
    let mut out = Box::new([0u8; SECTION_VOLUME]);
    if let Some(sky) = section.sky_light.as_ref() {
        write_nibbles(sky, &mut out, 0);
    }
    if let Some(block) = section.block_light.as_ref() {
        write_nibbles(block, &mut out, 4);
    }
    Some(out)
}

/// Nibble arrays are 2048 bytes; the low nibble of byte `i` is index `2i`.
fn write_nibbles(source: &[i8], out: &mut [u8; SECTION_VOLUME], shift: u32) {
    let count = source.len().min(SECTION_VOLUME / 2);
    for i in 0..count {
        let byte = source[i] as u8;
        out[i * 2] |= (byte & 0x0f) << shift;
        out[i * 2 + 1] |= (byte >> 4) << shift;
    }
}

/// A region holding one section, in the column at its own corner, filled with the named block.
///
/// Its palette puts the block at id 0 and air at id 1, the reverse of the order a world interns
/// them in, so a read that goes past the world's remap into the file's own numbering comes back
/// with the wrong block rather than with nothing.
#[cfg(test)]
pub fn one_section_region(name: &str) -> Region {
    let slots = REGION_CHUNKS * REGION_CHUNKS;
    let mut sections: Vec<Option<Section>> = (0..slots).map(|_| None).collect();
    sections[0] = Some(Section {
        blocks: Box::new([0; SECTION_VOLUME]),
        biomes: Box::new([0; 64]),
    });
    Region {
        states: vec![
            BlockStateKey {
                name: name.to_string(),
                props: Vec::new(),
            },
            BlockStateKey {
                name: "minecraft:air".to_string(),
                props: Vec::new(),
            },
        ],
        biomes: vec!["minecraft:plains".to_string()],
        air_state: 1,
        min_section_y: 0,
        sections_y: 1,
        sections,
        lights: (0..slots).map(|_| None).collect(),
        light_top: vec![1; slots],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_width_follows_the_anvil_rule() {
        assert_eq!(bits_per_entry(1), 4);
        assert_eq!(bits_per_entry(16), 4);
        assert_eq!(bits_per_entry(17), 5);
        assert_eq!(bits_per_entry(32), 5);
        assert_eq!(bits_per_entry(33), 6);
        assert_eq!(bits_per_entry(43), 6);
        assert_eq!(bits_per_entry(256), 8);
        assert_eq!(bits_per_entry(257), 9);
    }

    /// A region whose column (0, 0) stores light only in its middle section.
    fn one_lit_section() -> Region {
        let mut lights: Vec<Option<Box<[u8; SECTION_VOLUME]>>> =
            (0..REGION_CHUNKS * REGION_CHUNKS * 3).map(|_| None).collect();
        lights[1] = Some(Box::new([0x3a; SECTION_VOLUME]));
        let mut sections: Vec<Option<Section>> =
            (0..REGION_CHUNKS * REGION_CHUNKS * 3).map(|_| None).collect();
        sections[1] = Some(Section {
            blocks: Box::new([1; SECTION_VOLUME]),
            biomes: Box::new([0; 64]),
        });
        let mut light_top = vec![0u16; REGION_CHUNKS * REGION_CHUNKS];
        light_top[0] = 2;
        Region {
            states: Vec::new(),
            biomes: Vec::new(),
            air_state: 0,
            min_section_y: -1,
            sections_y: 3,
            sections,
            lights,
            light_top,
        }
    }

    /// A region whose every section holds one block state, so two of them can be told apart by
    /// what a read comes back with.
    fn uniform_region(name: &str) -> Region {
        let slots = REGION_CHUNKS * REGION_CHUNKS;
        Region {
            states: vec![
                BlockStateKey {
                    name: "minecraft:air".to_string(),
                    props: Vec::new(),
                },
                BlockStateKey {
                    name: name.to_string(),
                    props: Vec::new(),
                },
            ],
            biomes: vec![name.to_string()],
            air_state: 0,
            min_section_y: 0,
            sections_y: 1,
            sections: (0..slots)
                .map(|_| {
                    Some(Section {
                        blocks: Box::new([1; SECTION_VOLUME]),
                        biomes: Box::new([0; 64]),
                    })
                })
                .collect(),
            lights: (0..slots).map(|_| None).collect(),
            light_top: vec![0; slots],
        }
    }

    /// Each file interns its own ids from zero, so the same id means different blocks in two of
    /// them. Reading a merged world without remapping draws one region file entirely in the other
    /// one's blocks, which is a wrong frame that nothing fails on.
    #[test]
    fn a_window_reads_each_region_where_it_belongs_and_keeps_their_states_apart() {
        let mut world = World::new([-1, -1], [2, 2]);
        world.insert([-1, -1], uniform_region("minecraft:stone"));
        world.insert([0, 0], uniform_region("minecraft:dirt"));

        let id = |name: &str| {
            world
                .states
                .iter()
                .position(|state| state.name == name)
                .unwrap() as u16
        };
        let (stone, dirt) = (id("minecraft:stone"), id("minecraft:dirt"));
        assert_ne!(stone, dirt);

        let span = REGION_BLOCKS as i32;
        assert_eq!(world.block(span - 1, 0, span - 1), stone, "last block of r.-1.-1");
        assert_eq!(world.block(span, 0, span), dirt, "first block of r.0.0");
        assert_eq!(world.block(span, 0, 0), world.air_state, "a slot with no file in it");
        // The mesher reads one block outside the section it is meshing, and at the window's own
        // corner that coordinate is negative. Truncating division rounds it back into the first
        // region and quietly closes the outward faces of the whole corner.
        assert_eq!(world.block(-1, 0, 0), world.air_state, "past the window's corner");
        assert_eq!(world.light(-1, 0, 0), 0x0f, "open sky past the window's corner");
    }

    #[test]
    fn a_region_file_name_carries_signed_coordinates() {
        assert_eq!(region_coords("r.0.0.mca"), Some([0, 0]));
        assert_eq!(region_coords("r.-4.-4.mca"), Some([-4, -4]));
        assert_eq!(region_coords("r.-1.3.mca"), Some([-1, 3]));
        assert_eq!(region_coords("level.dat"), None);
    }

    /// The file drops a nibble array that is all zeroes, so a stored section without one is dark.
    /// Reading it as lit is what turns unlit caves fullbright.
    #[test]
    fn an_unlit_section_is_dark_and_only_the_sky_above_the_column_is_full() {
        let region = one_lit_section();
        assert_eq!(region.light(0, -16, 0), 0x00, "below the lit section");
        assert_eq!(region.light(0, 0, 0), 0x3a, "the lit section itself");
        assert_eq!(region.light(0, 16, 0), 0x0f, "open sky above the column");
        assert_eq!(region.light(0, -17, 0), 0x00, "below the stored world");
    }
}

