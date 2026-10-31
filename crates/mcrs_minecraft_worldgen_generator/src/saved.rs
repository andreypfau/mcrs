use mcrs_minecraft_core::RegionPos;
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use mcrs_voxel::ColumnPos;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use mcrs_minecraft_anvil::{Chunk, ChunkStatus, PaletteLookup, Properties, RegionFile, Section};
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_chunk::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_chunk::{VoxelId, VoxelPalette};
use mcrs_minecraft_registries::{Registries, Registry};
use std::time::Instant;

use mcrs_minecraft_dimension::Dimension;
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

impl PaletteLookup<VoxelId> for CorpusBlockStates<'_> {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<VoxelId> {
        let block = self.0.block(name)?;
        let mut id = block.default_state_id;
        for (property, text) in properties.iter() {
            id = block.with_text(id, property, text)?;
        }
        Some(id.number().into())
    }
}

/// Resolves a saved biome name against the registry the dimension runs with.
/// Biomes carry no properties, so only the name selects the entry.
pub struct RegistryBiomes<'a>(pub &'a Registry<BiomeFile>);

impl PaletteLookup<u8> for RegistryBiomes<'_> {
    fn resolve(&self, name: &str, _properties: Properties<'_>) -> Option<u8> {
        let name = ResourceLocationBuf::read(name).ok()?;
        self.0.by_name(&name).and_then(|id| id.narrow::<u8>().ok())
    }
}

/// The identifier's own characters allow a `..` path segment, which would leave
/// the world directory.
pub(crate) fn region_dir(world: &Path, dimension: &ResourceKeyBuf<Dimension>) -> Option<PathBuf> {
    if dimension
        .path()
        .split('/')
        .any(|segment| matches!(segment, "" | "." | ".."))
    {
        return None;
    }
    Some(
        world
            .join("dimensions")
            .join(dimension.namespace())
            .join(dimension.path())
            .join("region"),
    )
}

/// The region files of one dimension, kept open for as long as the dimension
/// runs.
///
/// A whole region file is read to answer for any of its 1024 columns, so the
/// bytes stay cached; a save large enough for that to matter needs an eviction
/// policy this does not have.
#[derive(Clone)]
pub struct SavedColumns(Arc<Regions>);

type Region = Arc<OnceLock<Option<Arc<RegionFile>>>>;

pub struct Regions {
    dir: PathBuf,
    registries: Registries,
    open: Mutex<HashMap<RegionPos, Region>>,
}

impl SavedColumns {
    /// `<world>/dimensions/<namespace>/<path>/region`, or `None` when this
    /// dimension has never been saved.
    pub fn open(
        world: &Path,
        dimension: &ResourceKeyBuf<Dimension>,
        registries: Registries,
    ) -> Option<Self> {
        let dir = region_dir(world, dimension)?;
        dir.is_dir().then(|| {
            Self(Arc::new(Regions {
                dir,
                registries,
                open: Mutex::new(HashMap::new()),
            }))
        })
    }

    pub fn registries(&self) -> &Registries {
        &self.0.registries
    }

    /// The saved column, or `None` when the save has never generated it.
    ///
    /// A column that is present but unreadable is reported and treated as
    /// absent: the alternative is killing the dimension over one bad chunk.
    ///
    /// A save also holds a ring of proto-chunks around what it generated,
    /// stopped at whatever status the player's view reached. Their sections
    /// hold no blocks, so anything short of `full` is absent too and the
    /// generator fills the column instead of the save handing back a hole.
    pub fn read(&self, pos: ColumnPos, blocks: &BlockDefinitions) -> Option<Chunk> {
        let Some(biomes) = self.0.registries.registry::<BiomeFile>() else {
            error!(?pos, "the dimension's registries hold no biomes");
            return None;
        };
        let region = self.region(RegionPos::from(pos))?;
        let chunk =
            match region.read_chunk(pos, &CorpusBlockStates(blocks), &RegistryBiomes(biomes)) {
                Ok(chunk) => chunk?,
                Err(err) => {
                    error!(%err, ?pos, "reading a saved column");
                    return None;
                }
            };
        let report = &chunk.decode_report;
        if !report.dropped_fluid_ticks.is_empty() {
            error!(?pos, dropped = ?report.dropped_fluid_ticks, "dropping saved fluid ticks that do not load");
        }
        if !report.dropped_structures.is_empty() {
            error!(?pos, dropped = ?report.dropped_structures, "dropping saved structure starts and references that do not load");
        }
        (chunk.status == ChunkStatus::Full).then_some(chunk)
    }

    /// Reading a region file is megabytes of blocking I/O, so the map lock is
    /// held only long enough to claim the slot — the read itself blocks nobody
    /// but the other readers of that same region.
    fn region(&self, at: RegionPos) -> Option<Arc<RegionFile>> {
        let slot = self.0.open.lock().unwrap().entry(at).or_default().clone();
        slot.get_or_init(|| {
            let path = self.0.dir.join(format!("r.{}.{}.mca", at.x, at.z));
            if !path.is_file() {
                return None;
            }
            let started = Instant::now();
            match RegionFile::open(&path) {
                Ok(region) => {
                    debug!(
                        region_x = at.x,
                        region_z = at.z,
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
/// section unloads the chunk instead of leaving it empty.
pub fn column_sections(mut saved: Vec<Section>, y_sections: &[i32]) -> Vec<Option<SectionData>> {
    y_sections
        .iter()
        .map(|&y| {
            let (blocks, biomes) = saved
                .iter()
                .position(|s| i32::from(s.y) == y)
                .map(|index| {
                    let section = saved.swap_remove(index);
                    (section.block_states, section.biomes)
                })
                .unwrap_or_default();
            Some((
                VoxelPalette(blocks.unwrap_or_default()),
                VoxelPalette(biomes.unwrap_or_default()),
            ))
        })
        .collect()
}

/// Blocks and biomes of one section, as the save holds them. The light the
/// save carries is not read: nothing writes a region file back, so every
/// session recomputes light from the blocks anyway.
pub type SectionData = (BlockPalette, BiomePalette);

/// The block entities a saved column carries, read back as the type that wrote
/// them rather than as a compound.
///
/// A kind [`GeneratedBlockEntity`] does not name is dropped, which is every
/// sign of a vanilla save; the ceiling lifts by widening that enum. A kind it
/// does name that fails to read is an error, never a silently missing entity.
pub fn saved_block_entities(
    chunk: &Chunk,
) -> Result<Vec<GeneratedBlockEntity>, mcrs_minecraft_nbt::Error> {
    chunk
        .block_entities
        .iter()
        .filter(|compound| {
            compound
                .get_string("id")
                .and_then(GeneratedBlockEntity::kind_of)
                .is_some()
        })
        .map(GeneratedBlockEntity::from_compound)
        .collect()
}
