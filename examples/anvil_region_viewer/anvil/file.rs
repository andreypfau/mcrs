use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;

use serde::Deserialize;

use super::{
    BlockStateKey, REGION_CHUNKS, Region, SECTION_VOLUME, Section, intern_state,
};

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
