use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod file;

pub use file::load;

pub const SECTION_SIZE: usize = 16;
pub const SECTION_VOLUME: usize = SECTION_SIZE * SECTION_SIZE * SECTION_SIZE;
pub const REGION_CHUNKS: usize = 32;

pub const REGION_BLOCKS: usize = REGION_CHUNKS * SECTION_SIZE;

pub const MIN_SECTION_Y: i32 = -4;
pub const SECTIONS_Y: usize = 24;

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

pub struct Section {
    pub blocks: Box<[u16; SECTION_VOLUME]>,
    pub biomes: Box<[u8; 64]>,
}

pub struct Region {
    pub states: Vec<BlockStateKey>,
    pub biomes: Vec<String>,
    pub air_state: u16,
    pub min_section_y: i32,
    pub sections_y: usize,
    sections: Vec<Option<Section>>,
    lights: Vec<Option<Box<[u8; SECTION_VOLUME]>>>,
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

pub struct Palette {
    pub states: Vec<BlockStateKey>,
    pub biomes: Vec<String>,
    intern: HashMap<BlockStateKey, u16>,
    biome_intern: HashMap<String, u8>,
}

impl Palette {
    pub const AIR: u16 = 0;

    pub fn new() -> Self {
        let mut palette = Self {
            states: Vec::new(),
            biomes: Vec::new(),
            intern: HashMap::new(),
            biome_intern: HashMap::new(),
        };
        let air = intern_state(
            &mut palette.intern,
            &mut palette.states,
            BlockStateKey {
                name: "minecraft:air".to_string(),
                props: Vec::new(),
            },
        );
        assert_eq!(air, Self::AIR);
        palette
    }

    fn absorb(&mut self, region: &Region) -> (Vec<u16>, Vec<u8>) {
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
        (states, biomes)
    }
}

#[derive(Clone)]
pub struct World {
    pub min_section: [i32; 3],
    pub sections: [usize; 3],
    pub min_region: [i32; 2],
    pub regions: [usize; 2],
    slots: Vec<Option<Arc<Resident>>>,
}

struct Resident {
    region: Region,
    states: Vec<u16>,
    biomes: Vec<u8>,
}

impl World {
    pub fn new(min_region: [i32; 2], regions: [usize; 2]) -> Self {
        Self {
            min_section: [
                min_region[0] * REGION_CHUNKS as i32,
                MIN_SECTION_Y,
                min_region[1] * REGION_CHUNKS as i32,
            ],
            sections: [regions[0] * REGION_CHUNKS, SECTIONS_Y, regions[1] * REGION_CHUNKS],
            min_region,
            regions,
            slots: (0..regions[0] * regions[1]).map(|_| None).collect(),
        }
    }

    pub fn insert(&mut self, palette: &mut Palette, coords: [i32; 2], region: Region) -> usize {
        let Some(slot) = self.slot_of(coords) else {
            return 0;
        };
        let (states, biomes) = palette.absorb(&region);

        let low = region.min_section_y;
        let high = low + region.sections_y as i32;
        let world_high = self.min_section[1] + self.sections[1] as i32;
        let outside = (self.min_section[1] - low).max(0) + (high - world_high).max(0);

        self.slots[slot] = Some(Arc::new(Resident {
            region,
            states,
            biomes,
        }));
        outside.max(0) as usize * REGION_CHUNKS * REGION_CHUNKS
    }

    pub fn holds(&self, coords: [i32; 2]) -> bool {
        match self.slot_of(coords) {
            Some(slot) => self.slots[slot].is_some(),
            None => true,
        }
    }

    fn slot_of(&self, coords: [i32; 2]) -> Option<usize> {
        let rx = coords[0] - self.min_region[0];
        let rz = coords[1] - self.min_region[1];
        if rx < 0 || rz < 0 || rx as usize >= self.regions[0] || rz as usize >= self.regions[1] {
            return None;
        }
        Some(rz as usize * self.regions[0] + rx as usize)
    }

    #[inline]
    fn resident(&self, x: i32, z: i32) -> Option<&Resident> {
        let span = REGION_BLOCKS as i32;
        let rx = x.div_euclid(span);
        let rz = z.div_euclid(span);
        if rx < 0 || rz < 0 || rx as usize >= self.regions[0] || rz as usize >= self.regions[1] {
            return None;
        }
        self.slots[rz as usize * self.regions[0] + rx as usize]
            .as_deref()
    }

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
            None => Palette::AIR,
        }
    }

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

    #[inline]
    pub fn section(&self, sx: usize, sy: usize, sz: usize) -> Option<&Section> {
        let chunks = REGION_CHUNKS;
        let resident = self.slots[sz / chunks * self.regions[0] + sx / chunks].as_deref()?;
        let local_y = self.min_section[1] + sy as i32 - resident.region.min_section_y;
        if local_y < 0 || local_y as usize >= resident.region.sections_y {
            return None;
        }
        resident
            .region
            .section(sx % chunks, local_y as usize, sz % chunks)
    }

    pub fn biome(&self, sx: usize, sy: usize, sz: usize, cell: usize) -> u8 {
        let chunks = REGION_CHUNKS;
        let Some(resident) = self.slots[sz / chunks * self.regions[0] + sx / chunks].as_deref()
        else {
            return 0;
        };
        match self.section(sx, sy, sz) {
            Some(section) => resident.biomes[section.biomes[cell] as usize],
            None => 0,
        }
    }

    pub fn states_reach(&self) -> usize {
        self.slots
            .iter()
            .flatten()
            .flat_map(|resident| resident.states.iter())
            .map(|id| *id as usize + 1)
            .max()
            .unwrap_or(0)
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

pub fn region_coords(name: &str) -> Option<[i32; 2]> {
    let (x, z) = name.strip_prefix("r.")?.strip_suffix(".mca")?.split_once('.')?;
    Some([x.parse().ok()?, z.parse().ok()?])
}

pub struct Window {
    pub min_region: [i32; 2],
    pub regions: [usize; 2],
    pub files: Vec<([i32; 2], PathBuf)>,
}

pub fn window(path: &Path, centre: [i32; 2], size: usize) -> Result<Window, String> {
    if !path.is_dir() {
        let coords = path
            .file_name()
            .and_then(|name| region_coords(&name.to_string_lossy()))
            .unwrap_or([0, 0]);
        return Ok(Window {
            min_region: coords,
            regions: [1, 1],
            files: vec![(coords, path.to_path_buf())],
        });
    }

    let min = [
        centre[0] - (size / 2) as i32,
        centre[1] - (size / 2) as i32,
    ];
    let mut files = Vec::new();
    for rz in 0..size as i32 {
        for rx in 0..size as i32 {
            let coords = [min[0] + rx, min[1] + rz];
            let file = path.join(format!("r.{}.{}.mca", coords[0], coords[1]));
            if file.is_file() {
                files.push((coords, file));
            }
        }
    }
    if files.is_empty() {
        return Err(format!(
            "{} holds none of the {size}x{size} region files around r.{}.{}",
            path.display(),
            centre[0],
            centre[1],
        ));
    }
    Ok(Window {
        min_region: min,
        regions: [size, size],
        files,
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

#[cfg(test)]
pub fn one_section_region(name: &str) -> Region {
    one_section_region_of(&[name], |_, _, _| 0)
}

#[cfg(test)]
pub fn one_section_region_of(
    names: &[&str],
    pick: impl Fn(usize, usize, usize) -> usize,
) -> Region {
    let slots = REGION_CHUNKS * REGION_CHUNKS;
    let mut sections: Vec<Option<Section>> = (0..slots).map(|_| None).collect();
    let mut blocks = Box::new([0u16; SECTION_VOLUME]);
    for y in 0..SECTION_SIZE {
        for z in 0..SECTION_SIZE {
            for x in 0..SECTION_SIZE {
                blocks[(y * SECTION_SIZE + z) * SECTION_SIZE + x] = pick(x, y, z) as u16;
            }
        }
    }
    sections[0] = Some(Section {
        blocks,
        biomes: Box::new([0; 64]),
    });
    let mut states: Vec<BlockStateKey> = names
        .iter()
        .map(|name| BlockStateKey {
            name: name.to_string(),
            props: Vec::new(),
        })
        .collect();
    let air_state = states.len() as u16;
    states.push(BlockStateKey {
        name: "minecraft:air".to_string(),
        props: Vec::new(),
    });
    Region {
        states,
        biomes: vec!["minecraft:plains".to_string()],
        air_state,
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

    #[test]
    fn a_window_reads_each_region_where_it_belongs_and_keeps_their_states_apart() {
        let mut palette = Palette::new();
        let mut world = World::new([-1, -1], [2, 2]);
        world.insert(&mut palette, [-1, -1], uniform_region("minecraft:stone"));
        world.insert(&mut palette, [0, 0], uniform_region("minecraft:dirt"));

        let id = |name: &str| {
            palette
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
        assert_eq!(world.block(span, 0, 0), Palette::AIR, "a slot with no file in it");
        assert_eq!(world.block(-1, 0, 0), Palette::AIR, "past the window's corner");
        assert_eq!(world.light(-1, 0, 0), 0x0f, "open sky past the window's corner");
    }

    #[test]
    fn a_region_file_name_carries_signed_coordinates() {
        assert_eq!(region_coords("r.0.0.mca"), Some([0, 0]));
        assert_eq!(region_coords("r.-4.-4.mca"), Some([-4, -4]));
        assert_eq!(region_coords("r.-1.3.mca"), Some([-1, 3]));
        assert_eq!(region_coords("level.dat"), None);
    }

    #[test]
    fn an_unlit_section_is_dark_and_only_the_sky_above_the_column_is_full() {
        let region = one_lit_section();
        assert_eq!(region.light(0, -16, 0), 0x00, "below the lit section");
        assert_eq!(region.light(0, 0, 0), 0x3a, "the lit section itself");
        assert_eq!(region.light(0, 16, 0), 0x0f, "open sky above the column");
        assert_eq!(region.light(0, -17, 0), 0x00, "below the stored world");
    }
}
