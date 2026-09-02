use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use bevy_ecs::prelude::Resource;
use mcrs_minecraft_anvil::{
    Biomes as SavedBiomes, BlockStateLookup, BlockStates, Chunk, ErrorKind, Light, Properties,
    RegionFile,
};
use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_core::RegistrySnapshot;
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_world::block::definition::BlockDefinitions;
use mcrs_voxel_light::storage::LightStorage;
use mcrs_voxel_light::{BlockLight, SkyLight};
use mcrs_voxel_storage::{PalettedContainer, VoxelId, VoxelPalette};
use std::time::Instant;

use tracing::{debug, error};

/// Resolves a saved palette entry against the corpus.
///
/// A save states every property as text. The type is never inferred from that
/// text — `"true"` and `"5"` are a string for any block that declares them as
/// one — so each declared value is rendered and compared instead, leaving the
/// corpus the authority on what a property holds.
///
/// A property the entry leaves out keeps the block's default value, which is
/// how a state that differs from the default in nothing is saved as a bare
/// name.
pub struct CorpusBlockStates<'a>(pub &'a BlockDefinitions);

impl BlockStateLookup for CorpusBlockStates<'_> {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<u32> {
        let block = self.0.block(name)?;
        let mut id = block.default_state_id;
        for (property, text) in properties.iter() {
            id = block.with_text(id, property, text)?;
        }
        Some(id.0 as u32)
    }
}

/// Resolves a saved biome name against the registry snapshot the dimension runs
/// with. Biomes carry no properties, so only the name selects the entry.
pub struct SnapshotBiomes<'a>(pub &'a RegistrySnapshot<Biome>);

impl BlockStateLookup for SnapshotBiomes<'_> {
    fn resolve(&self, name: &str, _properties: Properties<'_>) -> Option<u32> {
        self.0.by_location(name)
    }
}

/// The region files of one dimension, kept open for as long as the dimension
/// runs.
///
/// A whole region file is read to answer for any of its 1024 columns, so the
/// bytes stay cached; a save large enough for that to matter needs an eviction
/// policy this does not have.
#[derive(Resource, Clone)]
pub struct SavedColumns(Arc<Regions>);

type Region = Arc<OnceLock<Option<Arc<RegionFile>>>>;

pub struct Regions {
    dir: PathBuf,
    open: Mutex<HashMap<(i32, i32), Region>>,
}

impl SavedColumns {
    /// `<world>/dimensions/<namespace>/<path>/region`, or `None` when this
    /// dimension has never been saved.
    pub fn open(world: &Path, dimension: &str) -> Option<Self> {
        let (namespace, path) = dimension.split_once(':')?;
        let dir = world
            .join("dimensions")
            .join(namespace)
            .join(path)
            .join("region");
        dir.is_dir().then(|| {
            Self(Arc::new(Regions {
                dir,
                open: Mutex::new(HashMap::new()),
            }))
        })
    }

    /// The saved column, or `None` when the save has never generated it.
    ///
    /// A column that is present but unreadable is reported and treated as
    /// absent: the alternative is killing the dimension over one bad chunk.
    pub fn read(&self, x: i32, z: i32) -> Option<Chunk> {
        match self.region(x >> 5, z >> 5)?.read_chunk(x, z) {
            Ok(chunk) => chunk,
            Err(err) => {
                error!(%err, x, z, "reading a saved column");
                None
            }
        }
    }

    /// Reading a region file is megabytes of blocking I/O, so the map lock is
    /// held only long enough to claim the slot — the read itself blocks nobody
    /// but the other readers of that same region.
    fn region(&self, region_x: i32, region_z: i32) -> Option<Arc<RegionFile>> {
        let slot = self
            .0
            .open
            .lock()
            .unwrap()
            .entry((region_x, region_z))
            .or_default()
            .clone();
        slot.get_or_init(|| {
            let path = self.0.dir.join(format!("r.{region_x}.{region_z}.mca"));
            if !path.is_file() {
                return None;
            }
            let started = Instant::now();
            match RegionFile::open(&path) {
                Ok(region) => {
                    debug!(
                        region_x,
                        region_z,
                        ms = started.elapsed().as_secs_f32() * 1000.0,
                        "read a region file"
                    );
                    Some(Arc::new(region))
                }
                Err(err) => {
                    error!(%err, "opening a region file");
                    None
                }
            }
        })
        .clone()
    }
}

/// The requested sections of a saved column, in the order they were asked for.
///
/// A Y the save holds no section for is air rather than absent: an absent
/// section unloads the chunk instead of leaving it empty. Such a section sits
/// above everything the save wrote, so it takes full sky light.
pub fn column_sections(
    chunk: &Chunk,
    y_sections: &[i32],
    blocks: &BlockDefinitions,
    biomes: &RegistrySnapshot<Biome>,
) -> Result<Vec<Option<SectionData>>, ErrorKind> {
    y_sections
        .iter()
        .map(|&y| {
            let Some(section) = chunk.sections.iter().find(|s| i32::from(s.y) == y) else {
                return Ok(Some((
                    BlockPalette::default(),
                    BiomePalette::default(),
                    BlockLight::default(),
                    SkyLight(LightStorage::Uniform(15)),
                )));
            };
            Ok(Some((
                match &section.block_states {
                    Some(states) => block_palette(states, blocks)?,
                    None => BlockPalette::default(),
                },
                match &section.biomes {
                    Some(saved) => biome_palette(saved, biomes)?,
                    None => BiomePalette::default(),
                },
                BlockLight(saved_light(section.block_light.as_ref())),
                SkyLight(saved_light(section.sky_light.as_ref())),
            )))
        })
        .collect()
}

/// Blocks, biomes and both light layers of one section, as the save holds them.
pub type SectionData = (BlockPalette, BiomePalette, BlockLight, SkyLight);

/// The save's nibble array is laid out exactly like `LightNibbles`, so the
/// bytes move across whole.
fn saved_light(light: Option<&Light>) -> LightStorage {
    match light {
        Some(light) => LightStorage::from_nibbles(light.0.clone()),
        None => LightStorage::Empty,
    }
}

fn block_palette(
    states: &BlockStates,
    blocks: &BlockDefinitions,
) -> Result<BlockPalette, ErrorKind> {
    let ids: Vec<VoxelId> = states
        .resolve_palette(&CorpusBlockStates(blocks))?
        .into_iter()
        .map(|id| VoxelId(id as u16))
        .collect();
    let mut cells = vec![VoxelId::default(); BlockStates::ENTRY_COUNT];
    states.remap_into(&ids, &mut cells);
    Ok(VoxelPalette(PalettedContainer::from_cells(&cells)))
}

fn biome_palette(
    saved: &SavedBiomes,
    biomes: &RegistrySnapshot<Biome>,
) -> Result<BiomePalette, ErrorKind> {
    let ids: Vec<u8> = saved
        .resolve_palette(&SnapshotBiomes(biomes))?
        .into_iter()
        .map(|id| id as u8)
        .collect();
    let mut cells = vec![0u8; SavedBiomes::ENTRY_COUNT];
    saved.remap_into(&ids, &mut cells);
    Ok(VoxelPalette(PalettedContainer::from_cells(&cells)))
}
