use mcrs_voxel::SectionPos;

use crate::colors::{LightColors, LightType};
use crate::region::{REACH, SectionBricks, section_output};

pub const REGION_SIDE: u32 = SectionPos::SIZE as u32 + 2 * (REACH as u32 + 1);
pub const REGION_CELLS: usize = (REGION_SIDE * REGION_SIDE * REGION_SIDE) as usize;
pub const WAVES: usize = 15;
pub const MAX_LANES: usize = 64;
pub const SLOT_UNLOADED: u32 = u32::MAX;
pub const SLOT_ABOVE: u32 = u32::MAX - 1;

const LANE_WORDS: usize = 0;
const LANE_BASE: usize = 1;
const ATLAS: usize = 2;
const SLOTS: usize = ATLAS + 3;
const LANE_TABLE: usize = SLOTS + 27;
const COLOURS: usize = LANE_TABLE + 256 / 4;
pub const JOB_WORDS: usize = COLOURS + MAX_LANES;
const _: () = assert!(JOB_WORDS.is_multiple_of(4));

pub struct PackedBrick {
    pub words: Box<[u32]>,
    pub emitters: Vec<Emitter>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Emitter {
    pub light_type: LightType,
    pub level: u8,
    pub min: [u8; 3],
    pub max: [u8; 3],
}

pub fn pack(bricks: &SectionBricks) -> PackedBrick {
    let mut emitters: Vec<Emitter> = Vec::new();
    let words = bricks
        .entry
        .iter()
        .zip(&bricks.veto)
        .zip(&bricks.seeds)
        .enumerate()
        .map(|(local, ((&entry, &veto), &seed))| {
            assert!(entry <= 15, "entering a cell costs {entry}, past four bits");
            if seed.emission > 0 {
                let at = [local & 15, local >> 8, local >> 4 & 15].map(|c| c as u8);
                let found = emitters
                    .iter_mut()
                    .find(|e| e.light_type == seed.light_type && e.level == seed.emission);
                match found {
                    Some(e) => {
                        for ((min, max), at) in e.min.iter_mut().zip(&mut e.max).zip(at) {
                            *min = (*min).min(at);
                            *max = (*max).max(at);
                        }
                    }
                    None => emitters.push(Emitter {
                        light_type: seed.light_type,
                        level: seed.emission,
                        min: at,
                        max: at,
                    }),
                }
            }
            entry as u32
                | (veto as u32) << 4
                | (seed.emission as u32) << 8
                | (seed.light_type.0 as u32) << 16
        })
        .collect();
    emitters.sort_by_key(|e| (e.light_type, e.level));
    PackedBrick { words, emitters }
}

pub fn neighbours(section: SectionPos) -> impl Iterator<Item = SectionPos> {
    (-1..=1).flat_map(move |dy| {
        (-1..=1).flat_map(move |dz| {
            (-1..=1).map(move |dx| SectionPos::new(section.x + dx, section.y + dy, section.z + dz))
        })
    })
}

pub fn reaching_types<'a>(
    section: SectionPos,
    emitters: impl Fn(SectionPos) -> Option<&'a [Emitter]>,
) -> Vec<LightType> {
    let (min, size) = section_output(section);
    let (low, high) = (
        [min.x, min.y, min.z],
        [min.x, min.y, min.z].map(|c| c + size - 1),
    );
    let width = SectionPos::SIZE as i32;
    let mut types: Vec<LightType> = neighbours(section)
        .flat_map(|neighbour| {
            let base = [neighbour.x, neighbour.y, neighbour.z].map(|c| c * width);
            emitters(neighbour)
                .into_iter()
                .flatten()
                .filter(move |e| {
                    let gap: i32 = base
                        .into_iter()
                        .zip(e.min)
                        .zip(e.max)
                        .zip(low.into_iter().zip(high))
                        .map(|(((base, min), max), (low, high))| {
                            let lo = base + min as i32;
                            let hi = base + max as i32;
                            (lo - high).max(low - hi).max(0)
                        })
                        .sum();
                    gap < e.level as i32
                })
                .map(|e| e.light_type)
        })
        .collect();
    types.sort_unstable();
    types.dedup();
    types
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lane {
    pub light_type: LightType,
    pub colour: Option<[u8; 3]>,
}

pub fn lanes(types: &[LightType], colours: &LightColors) -> Vec<Lane> {
    assert!(
        types.len() <= MAX_LANES,
        "{} light types exceed {MAX_LANES} lanes",
        types.len()
    );
    types
        .iter()
        .map(|&light_type| Lane {
            light_type,
            colour: colours.rgb(light_type),
        })
        .collect()
}

pub fn lane_words(lanes: usize) -> u32 {
    lanes.div_ceil(4) as u32
}

pub fn job_words(
    lanes: &[Lane],
    slots: [u32; 27],
    lane_base: u32,
    atlas_origin: [u32; 3],
) -> [u32; JOB_WORDS] {
    assert!(
        lanes.len() <= MAX_LANES,
        "{} lanes exceed {MAX_LANES}",
        lanes.len()
    );
    let mut words = [0u32; JOB_WORDS];
    words[LANE_WORDS] = lane_words(lanes.len());
    words[LANE_BASE] = lane_base;
    words[ATLAS..SLOTS].copy_from_slice(&atlas_origin);
    words[SLOTS..LANE_TABLE].copy_from_slice(&slots);
    let mut table = [0xffu8; 256];
    for (lane, l) in lanes.iter().enumerate() {
        if let Some(slot) = table.get_mut(l.light_type.0 as usize) {
            *slot = lane as u8;
        }
    }
    let (table, _) = table.as_chunks::<4>();
    for (word, bytes) in words[LANE_TABLE..COLOURS].iter_mut().zip(table) {
        *word = u32::from_le_bytes(*bytes);
    }
    for (word, lane) in words[COLOURS..].iter_mut().zip(lanes) {
        *word = lane.colour.map_or(0, |[r, g, b]| {
            r as u32 | (g as u32) << 8 | (b as u32) << 16 | 1 << 24
        });
    }
    words
}
