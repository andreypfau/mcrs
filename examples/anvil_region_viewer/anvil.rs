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
    /// `block_light << 4 | sky_light`, or `None` when the file omits both arrays for this section.
    pub light: Option<Box<[u8; SECTION_VOLUME]>>,
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

    /// `block_light << 4 | sky_light` at a region-local block coordinate. Sections without light
    /// data read as fully sky-lit, which is what the vanilla client assumes above the world.
    #[inline]
    pub fn light(&self, x: i32, y: i32, z: i32) -> u8 {
        let sy = y.div_euclid(SECTION_SIZE as i32) - self.min_section_y;
        if x < 0
            || z < 0
            || x >= (REGION_CHUNKS * SECTION_SIZE) as i32
            || z >= (REGION_CHUNKS * SECTION_SIZE) as i32
            || sy < 0
            || sy as usize >= self.sections_y
        {
            return 0x0f;
        }
        let cx = x as usize / SECTION_SIZE;
        let cz = z as usize / SECTION_SIZE;
        match self.section(cx, sy as usize, cz).and_then(|s| s.light.as_ref()) {
            Some(light) => {
                let lx = x as usize % SECTION_SIZE;
                let lz = z as usize % SECTION_SIZE;
                let ly = y.rem_euclid(SECTION_SIZE as i32) as usize;
                light[ly * 256 + lz * SECTION_SIZE + lx]
            }
            None => 0x0f,
        }
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
    let mut sections: Vec<Option<Section>> =
        Vec::with_capacity(REGION_CHUNKS * REGION_CHUNKS * sections_y);
    sections.resize_with(REGION_CHUNKS * REGION_CHUNKS * sections_y, || None);

    for (slot, chunk) in chunks {
        let cx = slot % REGION_CHUNKS;
        let cz = slot / REGION_CHUNKS;
        for section in chunk.sections {
            let sy = section.y as i32 - min_section_y;
            if sy < 0 || sy as usize >= sections_y {
                continue;
            }
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
            sections[(cz * REGION_CHUNKS + cx) * sections_y + sy as usize] = Some(built);
        }
    }

    Ok(Region {
        states,
        biomes,
        air_state,
        min_section_y,
        sections_y,
        sections,
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
            light: unpack_light(section),
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
        light: unpack_light(section),
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

#[cfg(test)]
mod tests {
    use super::bits_per_entry;

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
}
