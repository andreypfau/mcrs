pub mod column_ticks;
mod quarantine;
pub mod stored_column;

use std::fmt;
use std::path::{Path, PathBuf};

use mcrs_engine_store::{ChunkVersion, DiskStore, VersionedChunk, keep_latest};
use mcrs_minecraft_anvil::{
    AnvilError, Chunk, ErrorKind, Heightmaps, PaletteLookup, PaletteNames, Properties, RegionFile,
    SavedBlockTick, SavedFluidTick, Section, write_chunk,
};
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::{BlockDefinitions, Blocks};
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_chunk::palette::BiomePalette;
use mcrs_minecraft_chunk::{
    BlockLight, ChunkBlocks, ColumnHeights, PalettedContainer, SkyLight, VoxelId, VoxelPalette,
};
use mcrs_minecraft_core::RegionPos;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_heightmap::{
    ColumnHeightmapSet, MotionHeightmap, NoLeavesHeightmap, SolidHeightmap, SurfaceHeightmap,
};
use mcrs_minecraft_registries::{Registries, Registry};
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use mcrs_minecraft_simulation::TickPriority;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, SectionPos};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::column_ticks::{ColumnTicks, StoredTick};
use crate::stored_column::{ChunkStatus, StoredColumn, StoredSection};

const OPEN_REGIONS: usize = 4;

const SECTIONS_PER_COLUMN: i32 = 16;

/// `<world>/dimensions/<namespace>/<path>/region`, or `None` when the key would leave the world.
pub fn region_dir(world: &Path, dimension: &ResourceKeyBuf<Dimension>) -> Option<PathBuf> {
    Some(key_dir(world, "dimensions", dimension)?.join("region"))
}

pub fn quarantine_dir(world: &Path, dimension: &ResourceKeyBuf<Dimension>) -> Option<PathBuf> {
    key_dir(world, "quarantine", dimension)
}

fn key_dir(world: &Path, root: &str, dimension: &ResourceKeyBuf<Dimension>) -> Option<PathBuf> {
    if dimension
        .path()
        .split('/')
        .any(|segment| matches!(segment, "" | "." | ".."))
    {
        return None;
    }
    Some(
        world
            .join(root)
            .join(dimension.namespace())
            .join(dimension.path()),
    )
}

/// The anvil files under one dimension, as the data layer over them.
pub struct RegionDisk {
    dir: PathBuf,
    blocks: Blocks,
    registries: Registries,
    open: FxHashMap<RegionPos, RegionFile>,
    pending: FxHashMap<ColumnPos, VersionedChunk<StoredColumn>>,
    durable: FxHashMap<ColumnPos, ChunkVersion>,
    quarantine: Option<PathBuf>,
    copied: FxHashSet<RegionPos>,
    fates: FxHashMap<ColumnPos, ColumnFate>,
}

/// What the disk knows about a column that it did not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColumnFate {
    /// The column failed to decode. It reads as missing until a replacement is written.
    Quarantined,
    /// The column was saved below `full`. It reads as missing, and its bytes stay on disk until
    /// the world produces a full column for it.
    BelowFull,
    /// The damaged region of the column could not be copied. The column is never read as missing
    /// and never written over.
    Unreadable,
}

impl RegionDisk {
    pub fn new(dir: PathBuf, blocks: Blocks, registries: Registries) -> Self {
        Self {
            dir,
            blocks,
            registries,
            open: FxHashMap::default(),
            pending: FxHashMap::default(),
            durable: FxHashMap::default(),
            quarantine: None,
            copied: FxHashSet::default(),
            fates: FxHashMap::default(),
        }
    }

    #[cfg(test)]
    fn open_regions(&self) -> usize {
        self.open.len()
    }

    pub fn with_quarantine(mut self, quarantine: PathBuf) -> Self {
        self.quarantine = Some(quarantine);
        self
    }

    fn path(&self, region: RegionPos) -> PathBuf {
        self.dir.join(format!("r.{}.{}.mca", region.x, region.z))
    }
}

impl DiskStore for RegionDisk {
    type Key = ColumnPos;
    type Chunk = StoredColumn;
    type Error = RegionDiskError;

    fn read(
        &mut self,
        key: ColumnPos,
    ) -> Result<Option<VersionedChunk<StoredColumn>>, Self::Error> {
        let at = RegionPos::from(key);
        if let Some(chunk) = self.pending.get(&key) {
            return Ok(Some(chunk.clone()));
        }
        match self.fates.get(&key) {
            Some(ColumnFate::Unreadable) => return Err(RegionDiskError::Unreadable(key)),
            Some(ColumnFate::Quarantined | ColumnFate::BelowFull) => return Ok(None),
            None => {}
        }
        let path = self.path(at);
        if !path.is_file() {
            return Ok(None);
        }
        if !self.open.contains_key(&at) {
            match RegionFile::open(&path) {
                Ok(region) => {
                    if self.open.len() >= OPEN_REGIONS
                        && let Some(oldest) = self.open.keys().next().copied()
                    {
                        self.open.remove(&oldest);
                    }
                    self.open.insert(at, region);
                }
                Err(error) => return self.quarantine(key, &error.to_string()),
            }
        }
        let biomes = self
            .registries
            .registry::<BiomeFile>()
            .ok_or(RegionDiskError::NoBiomes)?;
        let read = {
            let region = self.open.get(&at).ok_or(RegionDiskError::NoRegion(at))?;
            region.read_chunk(
                key,
                &CorpusBlockStates(&self.blocks),
                &RegistryBiomes(biomes),
            )
        };
        match read {
            Ok(Some(chunk)) if chunk.status == mcrs_minecraft_anvil::ChunkStatus::Full => {
                match stored(chunk) {
                    Ok(chunk) => Ok(Some(VersionedChunk {
                        version: self.durable.get(&key).copied().unwrap_or_default(),
                        chunk,
                    })),
                    Err(error) => self.quarantine(key, &error.to_string()),
                }
            }
            Ok(Some(_)) => {
                self.fates.insert(key, ColumnFate::BelowFull);
                Ok(None)
            }
            Ok(None) => Ok(None),
            Err(error) => self.quarantine(key, &error.to_string()),
        }
    }

    fn write(&mut self, key: ColumnPos, chunk: VersionedChunk<StoredColumn>) {
        if self
            .durable
            .get(&key)
            .is_none_or(|durable| *durable <= chunk.version)
        {
            keep_latest(&mut self.pending, key, chunk);
        }
    }

    /// A column whose region failed stays pending, and the next flush writes it again.
    fn flush(&mut self) -> Result<(), Self::Error> {
        let mut regions: FxHashMap<RegionPos, Vec<(ColumnPos, VersionedChunk<StoredColumn>)>> =
            FxHashMap::default();
        for (key, column) in std::mem::take(&mut self.pending) {
            regions
                .entry(RegionPos::from(key))
                .or_default()
                .push((key, column));
        }
        let mut failures = Vec::new();
        for (at, columns) in regions {
            let mut writable = Vec::with_capacity(columns.len());
            for (key, column) in columns {
                match self.fates.get(&key) {
                    Some(ColumnFate::Unreadable) => tracing::error!(
                        ?key,
                        "a column over an unreadable region is dropped, the region stays as it is"
                    ),
                    Some(ColumnFate::BelowFull) if column.chunk.status != ChunkStatus::Full => {}
                    _ => writable.push((key, column)),
                }
            }
            if writable.is_empty() {
                continue;
            }
            match self.write_region(at, &writable) {
                Ok(()) => {
                    for (key, column) in &writable {
                        self.fates.remove(key);
                        self.durable.insert(*key, column.version);
                    }
                }
                Err(error) => {
                    failures.push(error);
                    for (key, column) in writable {
                        keep_latest(&mut self.pending, key, column);
                    }
                }
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(RegionDiskError::Flush(failures))
        }
    }
}

impl RegionDisk {
    fn quarantine(
        &mut self,
        key: ColumnPos,
        reason: &str,
    ) -> Result<Option<VersionedChunk<StoredColumn>>, RegionDiskError> {
        let at = RegionPos::from(key);
        let region = self.open.remove(&at);
        match self.quarantine_region(at, region.as_ref(), reason.to_owned()) {
            Ok(()) => {
                self.fates.insert(key, ColumnFate::Quarantined);
                Ok(None)
            }
            Err(error) => {
                self.fates.insert(key, ColumnFate::Unreadable);
                Err(error)
            }
        }
    }

    fn quarantine_region(
        &mut self,
        at: RegionPos,
        region: Option<&RegionFile>,
        reason: String,
    ) -> Result<(), RegionDiskError> {
        let Some(root) = self.quarantine.as_deref() else {
            return Err(RegionDiskError::Quarantine {
                x: at.x,
                z: at.z,
                source: std::io::Error::other("no quarantine directory is set"),
            });
        };
        if self.copied.contains(&at) {
            return Ok(());
        }
        let externals = match region {
            Some(region) => region.external_columns(),
            None => quarantine::existing_externals(&self.dir, at),
        };
        match quarantine::quarantine_region(root, &self.dir, at, &externals, &reason) {
            Ok(_) => {
                self.copied.insert(at);
                Ok(())
            }
            Err(source) => Err(RegionDiskError::Quarantine {
                x: at.x,
                z: at.z,
                source,
            }),
        }
    }

    fn write_region(
        &mut self,
        at: RegionPos,
        columns: &[(ColumnPos, VersionedChunk<StoredColumn>)],
    ) -> Result<(), RegionDiskError> {
        std::fs::create_dir_all(&self.dir).map_err(|source| AnvilError {
            path: self.dir.clone(),
            kind: ErrorKind::Io(source),
        })?;
        let path = self.path(at);
        let mut region = match self.open.remove(&at) {
            Some(region) => region,
            None if path.is_file() => match RegionFile::open(&path) {
                Ok(region) => region,
                Err(_) if self.copied.contains(&at) => RegionFile::create(at, &path),
                Err(error) => {
                    self.quarantine_region(at, None, error.to_string())?;
                    RegionFile::create(at, &path)
                }
            },
            None => RegionFile::create(at, &path),
        };
        if region.has_damaged_slot() {
            self.quarantine_region(
                at,
                Some(&region),
                "a slot points outside the region file".to_owned(),
            )?;
        }
        let block_names = PaletteNames::<VoxelId>::new();
        let biome_names = PaletteNames::<u8>::new();
        let mut records = Vec::with_capacity(columns.len());
        let mut block_map = FxHashMap::default();
        let mut biome_map = FxHashMap::default();
        for (key, versioned) in columns {
            let chunk = &versioned.chunk;
            intern_blocks(chunk, &self.blocks, &block_names, &mut block_map)?;
            intern_biomes(chunk, &self.registries, &biome_names, &mut biome_map)?;
            let nbt = write_chunk(
                &chunk_of(*key, chunk, &block_map, &biome_map)?,
                chunk.min_section_y,
                &block_names,
                &biome_names,
            )?;
            records.push((*key, nbt));
        }
        region.write(&records, &path)?;
        Ok(())
    }
}

impl fmt::Debug for RegionDisk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegionDisk")
            .field("dir", &self.dir)
            .field("open", &self.open.len())
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RegionDiskError {
    #[error(transparent)]
    Anvil(#[from] AnvilError),
    #[error(transparent)]
    AnvilKind(#[from] ErrorKind),
    #[error("section Y {0} does not fit the save")]
    SectionY(i32),
    #[error("the registries hold no biomes")]
    NoBiomes,
    #[error("block state {0} is not in the block table")]
    UnknownBlock(u16),
    #[error("block tick {0} is not in the block table")]
    UnknownBlockTick(u16),
    #[error("fluid tick {0} is not in the fluid table")]
    UnknownFluidTick(u16),
    #[error("biome {0} is not in the biome registry")]
    UnknownBiome(u8),
    #[error("block entity {0} cannot be written")]
    BlockEntity(#[from] mcrs_minecraft_nbt::Error),
    #[error("region {0:?} is not open")]
    NoRegion(RegionPos),
    #[error("column {0:?} is unreadable and the copy of its region failed")]
    Unreadable(ColumnPos),
    #[error("a full column lacks one of its four heightmaps")]
    MissingHeightmaps,
    #[error("the container of a section cannot be built")]
    Container,
    #[error("{} region writes failed, the first: {}", .0.len(), .0[0])]
    Flush(Vec<RegionDiskError>),
    #[error("region {x},{z} cannot be quarantined: {source}")]
    Quarantine {
        x: i32,
        z: i32,
        source: std::io::Error,
    },
}

/// Resolves a saved block name and properties to a state of the block corpus.
pub struct CorpusBlockStates<'a>(pub &'a BlockDefinitions);

impl PaletteLookup<VoxelId> for CorpusBlockStates<'_> {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<VoxelId> {
        let block = self.0.block(name)?;
        let mut id = block.default_state_id;
        for (property, text) in properties.iter() {
            id = block.with_text(id, property, text)?;
        }
        Some(VoxelId::from(id))
    }
}

/// Resolves a saved biome name to its number in the biome registry.
pub struct RegistryBiomes<'a>(pub &'a Registry<BiomeFile>);

impl PaletteLookup<u8> for RegistryBiomes<'_> {
    fn resolve(&self, name: &str, _properties: Properties<'_>) -> Option<u8> {
        let name = ResourceLocationBuf::read(name).ok()?;
        self.0.by_name(&name).and_then(|id| id.narrow::<u8>().ok())
    }
}

fn intern_blocks(
    column: &StoredColumn,
    definitions: &BlockDefinitions,
    names: &PaletteNames<VoxelId>,
    map: &mut FxHashMap<u16, VoxelId>,
) -> Result<(), RegionDiskError> {
    for section in &column.sections {
        let mut states = Vec::new();
        section.blocks.for_each_distinct(|state| states.push(state));
        for state in states {
            if map.contains_key(&state.number()) {
                continue;
            }
            let (name, properties) = definitions
                .properties_of(state.number())
                .ok_or(RegionDiskError::UnknownBlock(state.number()))?;
            let is_default = definitions
                .block(name.as_str())
                .is_some_and(|block| block.default_state_id.number() == state.number());
            let id = names
                .intern(
                    name.as_str(),
                    properties
                        .iter()
                        .filter(|_| !is_default)
                        .map(|(key, value)| (key.as_str(), value.as_str())),
                )
                .ok_or(RegionDiskError::UnknownBlock(state.number()))?;
            map.insert(state.number(), id);
        }
    }
    Ok(())
}

fn intern_biomes(
    column: &StoredColumn,
    registries: &Registries,
    names: &PaletteNames<u8>,
    map: &mut FxHashMap<u8, u8>,
) -> Result<(), RegionDiskError> {
    let biomes = registries
        .registry::<BiomeFile>()
        .ok_or(RegionDiskError::NoBiomes)?;
    for section in &column.sections {
        let mut values = Vec::new();
        section.biomes.for_each_distinct(|value| values.push(value));
        for value in values {
            if map.contains_key(&value) {
                continue;
            }
            let id = Id::checked(u16::from(value), biomes.len())
                .ok_or(RegionDiskError::UnknownBiome(value))?;
            let name = biomes
                .name(id)
                .ok_or(RegionDiskError::UnknownBiome(value))?;
            let id = names
                .intern(name.as_str(), [])
                .ok_or(RegionDiskError::UnknownBiome(value))?;
            map.insert(value, id);
        }
    }
    Ok(())
}

fn chunk_of(
    pos: ColumnPos,
    column: &StoredColumn,
    block_map: &FxHashMap<u16, VoxelId>,
    biome_map: &FxHashMap<u8, u8>,
) -> Result<Chunk, RegionDiskError> {
    let no_light = |light: Option<&BlockLight>| light.map(|light| light.0.to_nibbles());
    let sky_light = |light: Option<&SkyLight>| light.map(|light| light.0.to_nibbles());
    let sections = column
        .sections
        .iter()
        .enumerate()
        .map(|(index, section)| {
            let y = column.min_section_y + index as i32;
            Ok(Section {
                y: i8::try_from(y).map_err(|_| RegionDiskError::SectionY(y))?,
                block_states: Some(container_of(&section.blocks, block_map)?),
                biomes: Some(container_of_biomes(&section.biomes, biome_map)?),
                noise_biomes: None,
                block_light: no_light(section.block_light.as_ref()),
                sky_light: sky_light(section.sky_light.as_ref()),
            })
        })
        .collect::<Result<Vec<_>, RegionDiskError>>()?;
    let mut block_entities = column
        .block_entities
        .iter()
        .map(mcrs_minecraft_nbt::to_nbt_compound)
        .collect::<Result<Vec<_>, _>>()?;
    block_entities.extend(column.unknown_block_entities.iter().cloned());
    let (block_ticks, fluid_ticks) = saved_ticks(&column.ticks)?;
    Ok(Chunk {
        pos,
        status: column.status,
        is_light_on: column.is_light_on,
        inhabited_time: column.inhabited_time,
        last_update: column.last_update,
        heightmaps: heightmaps_of(&column.heightmaps),
        block_entities,
        retrogen: column.retrogen.clone(),
        sections,
        fluid_ticks,
        block_ticks,
        structures: column.structures.clone(),
        post_processing: column.post_processing.clone(),
        upgrade_data: column.upgrade_data.clone(),
        blending_data: column.blending_data.clone(),
        decode_report: Default::default(),
    })
}

fn container_of(
    blocks: &ChunkBlocks,
    map: &FxHashMap<u16, VoxelId>,
) -> Result<PalettedContainer<VoxelId, { SectionPos::SIZE }>, RegionDiskError> {
    let mut distinct = Vec::new();
    blocks.for_each_distinct(|state| distinct.push(state));
    if let [only] = distinct[..] {
        let id = map
            .get(&only.number())
            .ok_or(RegionDiskError::UnknownBlock(only.number()))?;
        return Ok(PalettedContainer::Homogeneous(*id));
    }
    let mut cells = Vec::with_capacity(SectionPos::VOLUME);
    for y in 0..SectionPos::SIZE {
        for z in 0..SectionPos::SIZE {
            for x in 0..SectionPos::SIZE {
                let state = blocks.get_cell(x, y, z).ok_or(RegionDiskError::Container)?;
                cells.push(
                    *map.get(&state.number())
                        .ok_or(RegionDiskError::UnknownBlock(state.number()))?,
                );
            }
        }
    }
    PalettedContainer::from_cells(&cells).ok_or(RegionDiskError::Container)
}

fn container_of_biomes(
    biomes: &BiomePalette,
    map: &FxHashMap<u8, u8>,
) -> Result<PalettedContainer<u8, { SectionPos::SIZE }>, RegionDiskError> {
    let mut distinct = Vec::new();
    biomes.for_each_distinct(|value| distinct.push(value));
    if let [only] = distinct[..] {
        let id = map.get(&only).ok_or(RegionDiskError::UnknownBiome(only))?;
        return Ok(PalettedContainer::Homogeneous(*id));
    }
    let mut cells = Vec::with_capacity(SectionPos::VOLUME);
    for y in 0..SectionPos::SIZE {
        for z in 0..SectionPos::SIZE {
            for x in 0..SectionPos::SIZE {
                let value = biomes.get_cell(x, y, z).ok_or(RegionDiskError::Container)?;
                cells.push(
                    *map.get(&value)
                        .ok_or(RegionDiskError::UnknownBiome(value))?,
                );
            }
        }
    }
    PalettedContainer::from_cells(&cells).ok_or(RegionDiskError::Container)
}

fn stored_blocks(
    blocks: Option<PalettedContainer<VoxelId, { SectionPos::SIZE }>>,
) -> Result<ChunkBlocks, RegionDiskError> {
    let container = blocks.unwrap_or(PalettedContainer::Homogeneous(VoxelId(0)));
    let state = |voxel: VoxelId| {
        BlockStateId::checked(voxel.0, usize::MAX).ok_or(RegionDiskError::UnknownBlock(voxel.0))
    };
    if let PalettedContainer::Homogeneous(voxel) = container {
        return Ok(ChunkBlocks::new(VoxelPalette::homogeneous(state(voxel)?)));
    }
    let mut cells = Vec::with_capacity(SectionPos::VOLUME);
    for y in 0..SectionPos::SIZE {
        for z in 0..SectionPos::SIZE {
            for x in 0..SectionPos::SIZE {
                let voxel = container.get(x, y, z).ok_or(RegionDiskError::Container)?;
                cells.push(state(voxel)?);
            }
        }
    }
    Ok(ChunkBlocks::new(
        VoxelPalette::from_cells(&cells).ok_or(RegionDiskError::Container)?,
    ))
}

fn stored(chunk: Chunk) -> Result<StoredColumn, RegionDiskError> {
    if !chunk.decode_report.is_clean() {
        tracing::warn!(
            column = ?chunk.pos,
            report = ?chunk.decode_report,
            "entries of the column did not decode and are dropped from its next write"
        );
    }
    let ticks = ticks_of(chunk.block_ticks, chunk.fluid_ticks);
    let mut sections = chunk.sections;
    sections.sort_by_key(|section| section.y);
    let min_section_y = sections.first().map_or(0, |section| i32::from(section.y));
    let subsequent_y = sections.last().map_or(0, |section| i32::from(section.y)) + 1;
    let height = u32::try_from((subsequent_y - min_section_y).max(0) * SECTIONS_PER_COLUMN)
        .unwrap_or_default();
    let base = min_section_y * SECTIONS_PER_COLUMN;
    let heights = |raw: &Option<Vec<i64>>| {
        raw.as_deref()
            .and_then(|raw| ColumnHeights::from_raw(height, base, raw))
            .ok_or(RegionDiskError::MissingHeightmaps)
    };
    let heightmaps = ColumnHeightmapSet {
        surface: SurfaceHeightmap(heights(&chunk.heightmaps.world_surface)?),
        solid: SolidHeightmap(heights(&chunk.heightmaps.ocean_floor)?),
        motion: MotionHeightmap(heights(&chunk.heightmaps.motion_blocking)?),
        no_leaves: NoLeavesHeightmap(heights(&chunk.heightmaps.motion_blocking_no_leaves)?),
    };
    let mut block_entities = Vec::new();
    let mut unknown_block_entities = Vec::new();
    for compound in chunk.block_entities {
        if compound
            .get_string("id")
            .and_then(GeneratedBlockEntity::kind_of)
            .is_some()
        {
            block_entities.push(GeneratedBlockEntity::from_compound(&compound)?);
        } else {
            unknown_block_entities.push(compound);
        }
    }
    Ok(StoredColumn {
        status: chunk.status,
        is_light_on: chunk.is_light_on,
        inhabited_time: chunk.inhabited_time,
        last_update: chunk.last_update,
        min_section_y,
        sections: dense_sections(sections, min_section_y, subsequent_y)?,
        block_entities,
        unknown_block_entities,
        heightmaps,
        retrogen: chunk.retrogen,
        ticks,
        structures: chunk.structures,
        post_processing: chunk.post_processing,
        upgrade_data: chunk.upgrade_data,
        blending_data: chunk.blending_data,
    })
}

fn dense_sections(
    sections: Vec<Section>,
    min_section_y: i32,
    subsequent_y: i32,
) -> Result<Vec<StoredSection>, RegionDiskError> {
    let rows = usize::try_from(subsequent_y - min_section_y).unwrap_or_default();
    let mut rows_by_y: Vec<Option<Section>> = (0..rows).map(|_| None).collect();
    for section in sections {
        let index = usize::try_from(i32::from(section.y) - min_section_y).unwrap_or_default();
        if let Some(row) = rows_by_y.get_mut(index) {
            *row = Some(section);
        }
    }
    rows_by_y
        .into_iter()
        .map(|section| {
            let Some(section) = section else {
                return Ok(StoredSection {
                    blocks: stored_blocks(None)?,
                    biomes: VoxelPalette(PalettedContainer::Homogeneous(0)),
                    block_light: None,
                    sky_light: None,
                });
            };
            Ok(StoredSection {
                blocks: stored_blocks(section.block_states)?,
                biomes: VoxelPalette(section.biomes.unwrap_or(PalettedContainer::Homogeneous(0))),
                block_light: section.block_light.map(|light| {
                    BlockLight(mcrs_minecraft_chunk::LightStorage::from_nibbles(&light))
                }),
                sky_light: section.sky_light.map(|light| {
                    SkyLight(mcrs_minecraft_chunk::LightStorage::from_nibbles(&light))
                }),
            })
        })
        .collect()
}

fn saved_ticks(
    ticks: &ColumnTicks,
) -> Result<(Vec<SavedBlockTick>, Vec<SavedFluidTick>), RegionDiskError> {
    let mut blocks = Vec::with_capacity(ticks.block.len());
    for tick in &ticks.block {
        let block = Block::from_id(tick.kind)
            .ok_or(RegionDiskError::UnknownBlockTick(tick.kind.number()))?;
        blocks.push(SavedBlockTick {
            kind: block,
            pos: tick.pos,
            delay: tick.delay,
            priority: tick.priority as i32,
        });
    }
    let mut fluids = Vec::with_capacity(ticks.fluid.len());
    for tick in &ticks.fluid {
        let fluid = Fluid::from_id(tick.kind)
            .ok_or(RegionDiskError::UnknownFluidTick(tick.kind.number()))?;
        fluids.push(SavedFluidTick {
            kind: fluid,
            pos: tick.pos,
            delay: tick.delay,
            priority: tick.priority as i32,
        });
    }
    Ok((blocks, fluids))
}

fn ticks_of(block: Vec<SavedBlockTick>, fluid: Vec<SavedFluidTick>) -> ColumnTicks {
    fn stored_tick<T>(pos: BlockPos, kind: T, delay: i32, priority: i32) -> StoredTick<T> {
        StoredTick {
            pos,
            kind,
            delay,
            priority: TickPriority::from_saved(priority),
        }
    }
    ColumnTicks {
        block: block
            .into_iter()
            .map(|tick| stored_tick(tick.pos, tick.kind.id(), tick.delay, tick.priority))
            .collect(),
        fluid: fluid
            .into_iter()
            .map(|tick| stored_tick(tick.pos, tick.kind.id(), tick.delay, tick.priority))
            .collect(),
    }
}

fn heightmaps_of(maps: &ColumnHeightmapSet) -> Heightmaps {
    let longs = |heights: &ColumnHeights| {
        Some(
            heights
                .raw_longs()
                .iter()
                .map(|&long| long as i64)
                .collect::<Vec<_>>(),
        )
    };
    Heightmaps {
        world_surface: longs(&maps.surface.0),
        ocean_floor: longs(&maps.solid.0),
        motion_blocking: longs(&maps.motion.0),
        motion_blocking_no_leaves: longs(&maps.no_leaves.0),
        ..Heightmaps::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_engine_store::{ChunkVersion, DiskStore};
    use mcrs_minecraft_anvil::{ChunkStatus, SECTOR_BYTES};
    use mcrs_minecraft_block::block_keys::Block;
    use mcrs_minecraft_chunk::{LightStorage, VoxelPalette};
    use mcrs_minecraft_engine_registries::test_corpus;
    use mcrs_minecraft_game_registries::registry_load::test_registries;
    use mcrs_voxel::BlockPos;
    use std::path::Path;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("mcrs-region-disk-{}-{name}", std::process::id()))
    }

    fn full_maps() -> ColumnHeightmapSet {
        let mut maps = ColumnHeightmapSet::new(16, 0);
        for map in [
            &mut maps.surface.0,
            &mut maps.solid.0,
            &mut maps.motion.0,
            &mut maps.no_leaves.0,
        ] {
            for z in 0..16 {
                for x in 0..16 {
                    map.set(x, z, 16);
                }
            }
        }
        maps
    }

    fn column(block: Block, biome: u8, status: ChunkStatus) -> StoredColumn {
        let blocks = test_corpus().0.clone();
        let state = blocks.default_state_of(block.id());
        StoredColumn {
            status,
            is_light_on: true,
            inhabited_time: 12,
            last_update: 34,
            min_section_y: 0,
            sections: vec![StoredSection {
                blocks: ChunkBlocks::new(VoxelPalette::homogeneous(state)),
                biomes: VoxelPalette::homogeneous(biome),
                block_light: Some(BlockLight(LightStorage::Uniform(15))),
                sky_light: Some(SkyLight(LightStorage::Uniform(7))),
            }],
            block_entities: Vec::new(),
            unknown_block_entities: Vec::new(),
            heightmaps: full_maps(),
            retrogen: None,
            structures: Default::default(),
            post_processing: Vec::new(),
            upgrade_data: Default::default(),
            blending_data: None,
            ticks: Default::default(),
        }
    }

    #[test]
    fn a_column_written_to_a_region_reads_back_and_keeps_its_neighbour() {
        let dir = temp_dir("round_trip");
        let _ = std::fs::remove_dir_all(&dir);
        let blocks = test_corpus().0.clone();
        let registries = test_registries().clone();
        let biome = registries
            .registry::<BiomeFile>()
            .and_then(|registry| registry.iter().next())
            .and_then(|(id, _, _)| id.narrow::<u8>().ok())
            .expect("the test corpus has a biome");
        let first = ColumnPos::new(-1, -1);
        let second = ColumnPos::new(-1, -2);
        let mut disk = RegionDisk::new(dir.clone(), blocks.clone(), registries.clone());
        disk.write(
            first,
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column(Block::Stone, biome, ChunkStatus::Full),
            },
        );
        disk.write(
            second,
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column(Block::Dirt, biome, ChunkStatus::Full),
            },
        );

        disk.flush().unwrap();
        assert!(dir.join("r.-1.-1.mca").is_file());
        let mut disk = RegionDisk::new(dir.clone(), blocks, registries);
        let read = disk.read(first).unwrap().unwrap().chunk;
        assert_eq!(read.status, ChunkStatus::Full);
        assert_eq!(read.inhabited_time, 12);
        assert_eq!(read.last_update, 34);
        assert!(read.is_light_on);
        assert_eq!(
            read.sections[0].blocks.get_cell(0, 0, 0),
            Some(test_corpus().0.default_state_of(Block::Stone.id()))
        );
        let maps = read.heightmaps;
        for map in [
            &maps.surface.0,
            &maps.solid.0,
            &maps.motion.0,
            &maps.no_leaves.0,
        ] {
            assert_eq!(map.get(3, 5), 16);
        }
        assert_eq!(
            read.sections[0]
                .block_light
                .as_ref()
                .map(|light| light.0.get(0, 0, 0)),
            Some(15)
        );
        assert_eq!(
            read.sections[0]
                .sky_light
                .as_ref()
                .map(|light| light.0.get(0, 0, 0)),
            Some(7)
        );
        let neighbour = disk.read(second).unwrap().unwrap().chunk;
        assert_eq!(
            neighbour.sections[0].blocks.get_cell(0, 0, 0),
            Some(test_corpus().0.default_state_of(Block::Dirt.id()))
        );
        assert!(disk.read(ColumnPos::new(0, 0)).unwrap().is_none());
        let _ = std::fs::remove_dir_all(Path::new(&dir));
    }

    #[test]
    fn a_default_state_is_written_as_a_bare_id_and_any_other_with_all_its_properties() {
        use mcrs_minecraft_nbt::compound::NbtCompound;
        use mcrs_minecraft_nbt::tag::NbtTag;

        let dir = temp_dir("palette_shape");
        let _ = std::fs::remove_dir_all(&dir);
        let blocks = test_corpus().0.clone();
        let registries = test_registries().clone();
        let log = blocks
            .block("minecraft:oak_log")
            .expect("oak_log is in the corpus");
        let rotated = log
            .with_text(log.default_state_id, "axis", "x")
            .expect("oak_log has an axis");
        let mut cells = vec![log.default_state_id; SectionPos::VOLUME];
        cells[1] = rotated;
        let mut column = column(Block::Stone, 0, ChunkStatus::Full);
        column.sections[0].blocks = ChunkBlocks::new(VoxelPalette::from_cells(&cells).unwrap());
        column.sections[0].biomes = VoxelPalette::homogeneous(
            registries
                .registry::<BiomeFile>()
                .and_then(|registry| registry.iter().next())
                .and_then(|(id, _, _)| id.narrow::<u8>().ok())
                .expect("the test corpus has a biome"),
        );
        let mut disk = RegionDisk::new(dir.clone(), blocks, registries);
        let at = ColumnPos::new(0, 0);
        disk.write(
            at,
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column,
            },
        );
        disk.flush().unwrap();

        let region = mcrs_minecraft_anvil::RegionFile::open(dir.join("r.0.0.mca")).unwrap();
        let bytes = region.read_nbt(at).unwrap().unwrap();
        let root: NbtCompound =
            mcrs_minecraft_nbt::from_bytes(std::io::Cursor::new(&bytes[..])).unwrap();
        let sections = root.get_list("sections").unwrap();
        let section = sections[0].extract_compound().unwrap();
        let palette = section
            .get_compound("block_states")
            .unwrap()
            .get_list("palette")
            .unwrap();
        let mut bare = Vec::new();
        let mut states = Vec::new();
        for entry in palette {
            match entry {
                NbtTag::String(id) => bare.push(id.as_str()),
                NbtTag::Compound(state) => states.push(state),
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(bare, ["minecraft:oak_log"]);
        let [state] = states.as_slice() else {
            panic!("{states:?}");
        };
        assert_eq!(state.get_string("id"), Some("minecraft:oak_log"));
        let properties = state.get_compound("properties").unwrap();
        assert_eq!(properties.get_string("axis"), Some("x"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn full_chunk_with_maps(keys: &[&str]) -> Chunk {
        use mcrs_minecraft_core::VERSION;
        use mcrs_minecraft_nbt::compound::NbtCompound;
        use mcrs_minecraft_nbt::tag::NbtTag;

        let mut root = NbtCompound::new();
        root.put_int("DataVersion", VERSION.world_version);
        root.put_int("xPos", 0);
        root.put_int("zPos", 0);
        root.put_string("status", "minecraft:full".to_owned());
        let mut heightmaps = NbtCompound::new();
        for key in keys {
            heightmaps.put(
                key,
                NbtTag::LongArray(vec![0; ColumnHeights::new(16, 0).raw_longs().len()]),
            );
        }
        root.put_component("Heightmaps", heightmaps);
        let mut bytes = Vec::new();
        mcrs_minecraft_nbt::to_bytes(&root, &mut bytes).unwrap();
        let (blocks, biomes) = (PaletteNames::new(), PaletteNames::new());
        mcrs_minecraft_anvil::parse_chunk(&bytes, &blocks, &biomes).unwrap()
    }

    #[test]
    fn a_full_column_with_world_generation_heightmaps_loads_and_ignores_them() {
        let chunk = full_chunk_with_maps(&[
            "WORLD_SURFACE_WG",
            "OCEAN_FLOOR_WG",
            "WORLD_SURFACE",
            "OCEAN_FLOOR",
            "MOTION_BLOCKING",
            "MOTION_BLOCKING_NO_LEAVES",
        ]);
        assert!(stored(chunk).is_ok());
    }

    #[test]
    fn a_full_column_without_its_four_heightmaps_does_not_load() {
        let chunk = full_chunk_with_maps(&["WORLD_SURFACE_WG", "OCEAN_FLOOR_WG", "WORLD_SURFACE"]);
        assert!(matches!(
            stored(chunk),
            Err(RegionDiskError::MissingHeightmaps)
        ));
    }

    #[test]
    fn unknown_block_entities_and_structures_round_trip() {
        use mcrs_minecraft_anvil::{BlendingData, ChunkStructures, SavedPiece, StructureStart};
        use mcrs_minecraft_nbt::compound::NbtCompound;
        use mcrs_minecraft_resource::ResourceLocationBuf;
        use mcrs_minecraft_simulation::TickPriority;
        use std::collections::BTreeSet;

        let dir = temp_dir("extras");
        let _ = std::fs::remove_dir_all(&dir);
        let mut raw = NbtCompound::new();
        raw.put_string("id", "test:unknown".to_owned());
        raw.put_int("x", 1);
        raw.put_int("y", 2);
        raw.put_int("z", 3);
        let id = ResourceLocationBuf::read("minecraft:test_structure").unwrap();
        let piece = SavedPiece::BuriedTreasure {
            bounds: [0, 1, 2, 3, 4, 5],
            orientation: -1,
            gen_depth: 0,
        };
        let mut structures = ChunkStructures::default();
        structures.starts.insert(
            id.clone(),
            StructureStart {
                references: 1,
                pieces: vec![piece.clone()],
            },
        );
        structures
            .references
            .insert(id, BTreeSet::from([ColumnPos::new(4, 5)]));
        let mut chunk = column(Block::Stone, biome(), ChunkStatus::Full);
        chunk.unknown_block_entities.push(raw.clone());
        let spawner =
            GeneratedBlockEntity::mob_spawner(BlockPos::new(7, 8, 9), "minecraft:cave_spider");
        chunk.block_entities.push(spawner.clone());
        chunk.structures = structures.clone();
        chunk.post_processing = vec![vec![5], Vec::new()];
        chunk.upgrade_data.sides = 3;
        chunk.blending_data = Some(BlendingData {
            min_section: -4,
            max_section: 20,
            heights: None,
        });
        chunk.ticks.block.push(StoredTick {
            pos: BlockPos::new(3, 5, 4),
            kind: Block::Stone.id(),
            delay: -2,
            priority: TickPriority::High,
        });
        chunk.ticks.fluid.push(StoredTick {
            pos: BlockPos::new(5, 6, 7),
            kind: Fluid::Water.id(),
            delay: 9,
            priority: TickPriority::Low,
        });

        let blocks = test_corpus().0.clone();
        let registries = test_registries().clone();
        let mut disk = RegionDisk::new(dir.clone(), blocks.clone(), registries.clone());
        disk.write(
            ColumnPos::new(0, 0),
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk,
            },
        );
        disk.flush().unwrap();

        let mut disk = RegionDisk::new(dir.clone(), blocks, registries);
        let read = disk.read(ColumnPos::new(0, 0)).unwrap().unwrap().chunk;
        assert_eq!(read.unknown_block_entities, vec![raw]);
        assert_eq!(read.block_entities, vec![spawner]);
        assert_eq!(read.structures, structures);
        assert_eq!(read.post_processing, vec![vec![5], Vec::new()]);
        assert_eq!(read.upgrade_data.sides, 3);
        assert_eq!(
            read.blending_data,
            Some(BlendingData {
                min_section: -4,
                max_section: 20,
                heights: None,
            })
        );
        assert_eq!(
            read.ticks.block,
            vec![StoredTick {
                pos: BlockPos::new(3, 5, 4),
                kind: Block::Stone.id(),
                delay: -2,
                priority: TickPriority::High,
            }]
        );
        assert_eq!(
            read.ticks.fluid,
            vec![StoredTick {
                pos: BlockPos::new(5, 6, 7),
                kind: Fluid::Water.id(),
                delay: 9,
                priority: TickPriority::Low,
            }]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn disk_with_quarantine(dir: &Path) -> RegionDisk {
        RegionDisk::new(
            dir.join("region"),
            test_corpus().0.clone(),
            test_registries().clone(),
        )
        .with_quarantine(dir.join("quarantine"))
    }

    fn biome() -> u8 {
        test_registries()
            .registry::<BiomeFile>()
            .and_then(|registry| registry.iter().next())
            .and_then(|(id, _, _)| id.narrow::<u8>().ok())
            .expect("the test corpus has a biome")
    }

    #[test]
    fn a_damaged_region_is_copied_before_a_new_column_replaces_it() {
        let dir = temp_dir("quarantine");
        let _ = std::fs::remove_dir_all(&dir);
        let pos = ColumnPos::new(0, 0);
        let mut disk = disk_with_quarantine(&dir);
        disk.write(
            pos,
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column(Block::Stone, biome(), ChunkStatus::Full),
            },
        );
        disk.flush().unwrap();
        let region = dir.join("region/r.0.0.mca");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&region)
            .unwrap();
        file.set_len(2 * SECTOR_BYTES as u64).unwrap();
        drop(file);

        let mut disk = disk_with_quarantine(&dir);
        assert!(disk.read(pos).unwrap().is_none());
        let attempt = dir.join("quarantine/r.0.0/attempt-1");
        assert!(attempt.join("r.0.0.mca").is_file());
        assert!(attempt.join("report.txt").is_file());
        assert!(disk.read(pos).unwrap().is_none());
        assert!(!dir.join("quarantine/r.0.0/attempt-2").exists());

        disk.write(
            pos,
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column(Block::Dirt, biome(), ChunkStatus::Full),
            },
        );
        disk.flush().unwrap();
        let read = disk.read(pos).unwrap().unwrap().chunk;
        assert_eq!(
            read.sections[0].blocks.get_cell(0, 0, 0),
            Some(test_corpus().0.default_state_of(Block::Dirt.id()))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_column_of_another_version_is_quarantined() {
        use mcrs_minecraft_anvil::RegionFile;
        use mcrs_minecraft_nbt::Nbt;
        use mcrs_minecraft_nbt::compound::NbtCompound;

        let dir = temp_dir("old_version");
        let _ = std::fs::remove_dir_all(&dir);
        let pos = ColumnPos::new(0, 0);
        let mut root = NbtCompound::new();
        root.put_int("DataVersion", 1);
        root.put_int("xPos", 0);
        root.put_int("zPos", 0);
        root.put_int("yPos", 0);
        root.put_string("status", "minecraft:full".to_owned());
        let bytes = Nbt::new(String::new(), root).write().unwrap();
        let region_dir = dir.join("region");
        std::fs::create_dir_all(&region_dir).unwrap();
        let path = region_dir.join("r.0.0.mca");
        RegionFile::create(RegionPos::new(0, 0), &path)
            .write(&[(pos, bytes.to_vec())], &path)
            .unwrap();

        let mut disk = disk_with_quarantine(&dir);
        assert!(disk.read(pos).unwrap().is_none());
        assert!(dir.join("quarantine/r.0.0/attempt-1/r.0.0.mca").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_region_flush_keeps_the_column_for_the_next_flush() {
        let dir = temp_dir("flush_failure");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::write(&dir, b"").unwrap();
        let mut disk = RegionDisk::new(
            dir.clone(),
            test_corpus().0.clone(),
            test_registries().clone(),
        );
        let pos = ColumnPos::new(0, 0);
        disk.write(
            pos,
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column(Block::Stone, biome(), ChunkStatus::Full),
            },
        );
        assert!(disk.flush().is_err());
        assert!(disk.read(pos).unwrap().is_some());

        std::fs::remove_file(&dir).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        disk.flush().unwrap();
        assert!(disk.pending.is_empty());
        assert!(disk.read(pos).unwrap().is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_column_that_is_not_full_reads_as_missing_and_keeps_its_bytes() {
        let dir = temp_dir("not_full");
        let _ = std::fs::remove_dir_all(&dir);
        let pos = ColumnPos::new(0, 0);
        let mut disk = disk_with_quarantine(&dir);
        disk.write(pos, versioned(Block::Stone, ChunkStatus::NoiseBiomes));
        disk.flush().unwrap();
        let region = dir.join("region/r.0.0.mca");
        let saved = std::fs::read(&region).unwrap();

        let mut disk = disk_with_quarantine(&dir);
        assert!(disk.read(pos).unwrap().is_none());
        assert!(!dir.join("quarantine").exists());
        disk.write(pos, versioned(Block::Dirt, ChunkStatus::NoiseBiomes));
        disk.flush().unwrap();
        assert_eq!(std::fs::read(&region).unwrap(), saved);

        disk.write(pos, versioned(Block::Dirt, ChunkStatus::Full));
        disk.flush().unwrap();
        let read = disk.read(pos).unwrap().unwrap().chunk;
        assert_eq!(read.status, ChunkStatus::Full);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn versioned(block: Block, status: ChunkStatus) -> VersionedChunk<StoredColumn> {
        VersionedChunk {
            version: ChunkVersion::default(),
            chunk: column(block, biome(), status),
        }
    }

    #[test]
    fn a_bad_column_does_not_hide_its_neighbour() {
        use mcrs_minecraft_nbt::Nbt;
        use mcrs_minecraft_nbt::compound::NbtCompound;

        let dir = temp_dir("bad_column");
        let _ = std::fs::remove_dir_all(&dir);
        let bad = ColumnPos::new(0, 0);
        let good = ColumnPos::new(1, 0);
        let mut disk = disk_with_quarantine(&dir);
        disk.write(bad, versioned(Block::Stone, ChunkStatus::Full));
        disk.write(good, versioned(Block::Dirt, ChunkStatus::Full));
        disk.flush().unwrap();
        let path = dir.join("region/r.0.0.mca");
        let mut root = NbtCompound::new();
        root.put_int("DataVersion", 1);
        let damaged = Nbt::new(String::new(), root).write().unwrap();
        RegionFile::open(&path)
            .unwrap()
            .write(&[(bad, damaged.to_vec())], &path)
            .unwrap();

        let mut disk = disk_with_quarantine(&dir);
        assert!(disk.read(bad).unwrap().is_none());
        assert!(disk.read(good).unwrap().is_some());
        assert!(!dir.join("quarantine/r.0.0/attempt-2").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failing_region_does_not_stop_the_flush_and_the_newest_column_wins() {
        let dir = temp_dir("flush_continues");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("r.1.0.mca")).unwrap();
        let mut disk = RegionDisk::new(
            dir.clone(),
            test_corpus().0.clone(),
            test_registries().clone(),
        );
        let ok = ColumnPos::new(0, 0);
        let failing = ColumnPos::new(32, 0);
        disk.write(ok, versioned(Block::Stone, ChunkStatus::Full));
        disk.write(failing, versioned(Block::Stone, ChunkStatus::Full));
        assert!(matches!(
            disk.flush(),
            Err(RegionDiskError::Flush(failures)) if failures.len() == 1
        ));
        assert!(dir.join("r.0.0.mca").is_file());
        assert!(disk.pending.contains_key(&failing) && !disk.pending.contains_key(&ok));

        disk.write(failing, versioned(Block::Dirt, ChunkStatus::Full));
        let read = disk.read(failing).unwrap().unwrap().chunk;
        assert_eq!(
            read.sections[0].blocks.get_cell(0, 0, 0),
            Some(test_corpus().0.default_state_of(Block::Dirt.id()))
        );
        std::fs::remove_dir_all(dir.join("r.1.0.mca")).unwrap();
        disk.flush().unwrap();
        let mut disk = RegionDisk::new(
            dir.clone(),
            test_corpus().0.clone(),
            test_registries().clone(),
        );
        let read = disk.read(failing).unwrap().unwrap().chunk;
        assert_eq!(
            read.sections[0].blocks.get_cell(0, 0, 0),
            Some(test_corpus().0.default_state_of(Block::Dirt.id()))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_region_that_cannot_be_opened_is_replaced_after_its_copy() {
        let dir = temp_dir("short_header");
        let _ = std::fs::remove_dir_all(&dir);
        let pos = ColumnPos::new(0, 0);
        std::fs::create_dir_all(dir.join("region")).unwrap();
        std::fs::write(dir.join("region/r.0.0.mca"), [0u8; 2]).unwrap();
        let mut disk = disk_with_quarantine(&dir);
        assert!(disk.read(pos).unwrap().is_none());
        assert!(dir.join("quarantine/r.0.0/attempt-1/r.0.0.mca").is_file());

        disk.write(
            pos,
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column(Block::Dirt, biome(), ChunkStatus::Full),
            },
        );
        disk.flush().unwrap();
        let read = disk.read(pos).unwrap().unwrap().chunk;
        assert_eq!(
            read.sections[0].blocks.get_cell(0, 0, 0),
            Some(test_corpus().0.default_state_of(Block::Dirt.id()))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_flush_copies_a_region_that_fails_to_open_before_replacing_it() {
        let dir = temp_dir("flush_short_header");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("region")).unwrap();
        std::fs::write(dir.join("region/r.0.0.mca"), [0u8; 2]).unwrap();
        std::fs::write(dir.join("region/c.5.5.mcc"), b"external").unwrap();
        let mut disk = disk_with_quarantine(&dir);
        disk.write(
            ColumnPos::new(0, 0),
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column(Block::Dirt, biome(), ChunkStatus::Full),
            },
        );
        disk.flush().unwrap();
        let attempt = dir.join("quarantine/r.0.0/attempt-1");
        assert!(attempt.join("r.0.0.mca").is_file());
        assert!(attempt.join("c.5.5.mcc").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_flush_copies_a_region_with_a_damaged_slot_before_rewriting_it() {
        let dir = temp_dir("flush_damaged_slot");
        let _ = std::fs::remove_dir_all(&dir);
        let mut disk = disk_with_quarantine(&dir);
        disk.write(
            ColumnPos::new(0, 0),
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column(Block::Stone, biome(), ChunkStatus::Full),
            },
        );
        disk.flush().unwrap();
        let region = dir.join("region/r.0.0.mca");
        let mut bytes = std::fs::read(&region).unwrap();
        bytes[4..8].copy_from_slice(&((900u32 << 8) | 1).to_be_bytes());
        std::fs::write(&region, bytes).unwrap();

        let mut disk = disk_with_quarantine(&dir);
        disk.write(
            ColumnPos::new(2, 0),
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column(Block::Dirt, biome(), ChunkStatus::Full),
            },
        );
        disk.flush().unwrap();
        assert!(
            dir.join("quarantine/r.0.0/attempt-1/r.0.0.mca").is_file(),
            "the damaged slot must be copied before the rewrite drops it"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_region_that_cannot_be_copied_is_not_read_as_missing() {
        let dir = temp_dir("quarantine_failure");
        let _ = std::fs::remove_dir_all(&dir);
        let pos = ColumnPos::new(0, 0);
        let mut disk = disk_with_quarantine(&dir);
        disk.write(
            pos,
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column(Block::Stone, biome(), ChunkStatus::Full),
            },
        );
        disk.flush().unwrap();
        let region = dir.join("region/r.0.0.mca");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&region)
            .unwrap();
        file.set_len(2 * SECTOR_BYTES as u64).unwrap();
        drop(file);
        std::fs::write(dir.join("quarantine"), b"").unwrap();

        let mut disk = disk_with_quarantine(&dir);
        assert!(
            disk.read(pos).is_err(),
            "a failed copy must not look missing"
        );
        disk.write(
            pos,
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column(Block::Dirt, biome(), ChunkStatus::Full),
            },
        );
        disk.flush().unwrap();
        assert!(disk.pending.is_empty());
        assert_eq!(
            std::fs::metadata(&region).unwrap().len(),
            2 * SECTOR_BYTES as u64
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_region_with_no_quarantine_directory_is_never_written_over() {
        let dir = temp_dir("no_quarantine");
        let _ = std::fs::remove_dir_all(&dir);
        let pos = ColumnPos::new(0, 0);
        let mut disk = disk_with_quarantine(&dir);
        disk.write(
            pos,
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column(Block::Stone, biome(), ChunkStatus::Full),
            },
        );
        disk.flush().unwrap();
        let region = dir.join("region/r.0.0.mca");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&region)
            .unwrap();
        file.set_len(2 * SECTOR_BYTES as u64).unwrap();
        drop(file);

        let mut disk = RegionDisk::new(
            dir.join("region"),
            test_corpus().0.clone(),
            test_registries().clone(),
        );
        assert!(disk.read(pos).is_err());
        disk.write(
            pos,
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk: column(Block::Dirt, biome(), ChunkStatus::Full),
            },
        );
        disk.flush().unwrap();
        assert_eq!(
            std::fs::metadata(&region).unwrap().len(),
            2 * SECTOR_BYTES as u64
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn old_region_handles_are_released() {
        let dir = temp_dir("release");
        let _ = std::fs::remove_dir_all(&dir);
        let mut disk = disk_with_quarantine(&dir);
        for x in 0..5 {
            disk.write(
                ColumnPos::new(x * 32, 0),
                VersionedChunk {
                    version: ChunkVersion::default(),
                    chunk: column(Block::Stone, biome(), ChunkStatus::Full),
                },
            );
        }
        disk.flush().unwrap();
        for x in 0..5 {
            assert!(disk.read(ColumnPos::new(x * 32, 0)).unwrap().is_some());
            assert!(disk.open_regions() <= OPEN_REGIONS);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_region_is_not_quarantined() {
        let dir = temp_dir("missing");
        let _ = std::fs::remove_dir_all(&dir);
        let mut disk = disk_with_quarantine(&dir);
        assert!(disk.read(ColumnPos::new(0, 0)).unwrap().is_none());
        assert!(!dir.join("quarantine").exists());
    }
    #[test]
    fn a_column_on_each_side_of_a_region_boundary_lands_in_its_own_file() {
        let dir = temp_dir("boundary");
        let _ = std::fs::remove_dir_all(&dir);
        let mut disk = RegionDisk::new(
            dir.clone(),
            test_corpus().0.clone(),
            test_registries().clone(),
        );
        let columns = [
            ColumnPos::new(31, 31),
            ColumnPos::new(32, 32),
            ColumnPos::new(-1, -1),
        ];
        for pos in columns {
            disk.write(
                pos,
                VersionedChunk {
                    version: ChunkVersion::default(),
                    chunk: column(Block::Stone, biome(), ChunkStatus::Full),
                },
            );
        }
        disk.flush().unwrap();
        assert!(dir.join("r.0.0.mca").is_file());
        assert!(dir.join("r.1.1.mca").is_file());
        assert!(dir.join("r.-1.-1.mca").is_file());
        for pos in columns {
            assert!(disk.read(pos).unwrap().is_some(), "{pos:?} reads back");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn quarantine_copies_an_external_column_beside_its_region() {
        let dir = temp_dir("quarantine_external");
        let _ = std::fs::remove_dir_all(&dir);
        let region_dir = dir.join("region");
        std::fs::create_dir_all(&region_dir).unwrap();
        let mut region = vec![0u8; 3 * SECTOR_BYTES];
        region[..3].copy_from_slice(&[0, 0, 2]);
        region[3] = 1;
        let stub = 2 * SECTOR_BYTES;
        region[stub..stub + 4].copy_from_slice(&1i32.to_be_bytes());
        region[stub + 4] = 2 | 128;
        std::fs::write(region_dir.join("r.0.0.mca"), region).unwrap();
        std::fs::write(region_dir.join("c.0.0.mcc"), b"not a compressed chunk").unwrap();

        let mut disk = disk_with_quarantine(&dir);
        assert!(disk.read(ColumnPos::new(0, 0)).unwrap().is_none());
        let attempt = dir.join("quarantine/r.0.0/attempt-1");
        assert!(attempt.join("r.0.0.mca").is_file());
        assert!(attempt.join("c.0.0.mcc").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn test_biome() -> u8 {
        test_registries()
            .registry::<BiomeFile>()
            .and_then(|registry| registry.iter().next())
            .and_then(|(id, _, _)| id.narrow::<u8>().ok())
            .expect("the test corpus has a biome")
    }

    fn at_version(version: u64, block: Block) -> VersionedChunk<StoredColumn> {
        VersionedChunk {
            version: ChunkVersion(version),
            chunk: column(block, test_biome(), ChunkStatus::Full),
        }
    }

    #[test]
    fn a_saved_column_with_sparse_unsorted_sections_keeps_each_section_at_its_y() {
        let blocks = test_corpus().0.clone();
        let registries = test_registries().clone();
        let block_names = PaletteNames::<VoxelId>::new();
        let biome_names = PaletteNames::<u8>::new();
        let section_at = |block: Block, y: i8| {
            let column = column(block, test_biome(), ChunkStatus::Full);
            let (mut block_map, mut biome_map) = (FxHashMap::default(), FxHashMap::default());
            intern_blocks(&column, &blocks, &block_names, &mut block_map).unwrap();
            intern_biomes(&column, &registries, &biome_names, &mut biome_map).unwrap();
            let mut chunk =
                chunk_of(ColumnPos::new(0, 0), &column, &block_map, &biome_map).unwrap();
            let mut section = chunk.sections.remove(0);
            section.y = y;
            section.block_states = Some(PalettedContainer::Homogeneous(VoxelId(
                blocks.default_state_of(block.id()).number(),
            )));
            (section, chunk)
        };
        let (dirt, _) = section_at(Block::Dirt, 2);
        let (stone, mut chunk) = section_at(Block::Stone, 0);
        chunk.sections = vec![dirt, stone];
        chunk.heightmaps = heightmaps_of(&ColumnHeightmapSet::new(48, 0));

        let read = stored(chunk).unwrap();

        assert_eq!(read.min_section_y, 0);
        assert_eq!(read.sections.len(), 3);
        let state = |block: Block| Some(test_corpus().0.default_state_of(block.id()));
        assert_eq!(
            read.sections[0].blocks.get_cell(0, 0, 0),
            state(Block::Stone)
        );
        assert_eq!(read.sections[1].blocks.get_cell(0, 0, 0), state(Block::Air));
        assert_eq!(
            read.sections[2].blocks.get_cell(0, 0, 0),
            state(Block::Dirt)
        );
    }

    #[test]
    fn an_older_write_after_a_flush_does_not_replace_a_newer_column() {
        let dir = temp_dir("older_after_flush");
        let _ = std::fs::remove_dir_all(&dir);
        let key = ColumnPos::new(0, 0);
        let mut disk = RegionDisk::new(
            dir.clone(),
            test_corpus().0.clone(),
            test_registries().clone(),
        );
        disk.write(key, at_version(2, Block::Stone));
        disk.flush().unwrap();
        disk.write(key, at_version(1, Block::Dirt));
        disk.flush().unwrap();

        let read = disk.read(key).unwrap().unwrap();

        assert_eq!(read.version, ChunkVersion(2));
        assert_eq!(
            read.chunk.sections[0].blocks.get_cell(0, 0, 0),
            Some(test_corpus().0.default_state_of(Block::Stone.id()))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
