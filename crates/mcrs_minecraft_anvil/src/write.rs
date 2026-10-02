use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;

use mcrs_minecraft_chunk::section::{Biomes, Blocks, NoiseBiomes};
use mcrs_minecraft_chunk::{PalettedContainer, SectionKind, VoxelId};
use mcrs_minecraft_core::VERSION;

use crate::ErrorKind;
use crate::chunk::{Chunk, PackedData, RawChunk, RawLight, RawPalettedContainer, RawSection};
use crate::palette::{BlockStateList, PaletteLookup, Properties};

/// An id a [`PaletteNames`] can hand out; the width bounds how many distinct
/// entries one table holds.
pub trait PaletteId: Copy {
    fn from_index(index: usize) -> Option<Self>;
    fn index(self) -> usize;
}

impl PaletteId for VoxelId {
    fn from_index(index: usize) -> Option<Self> {
        u16::try_from(index).ok().map(VoxelId)
    }

    fn index(self) -> usize {
        self.0 as usize
    }
}

impl PaletteId for u8 {
    fn from_index(index: usize) -> Option<Self> {
        u8::try_from(index).ok()
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// Names a palette entry by its own text instead of a registry, so a chunk read
/// through it writes back with every name and property order it was read with.
/// Ids are handed out in the order entries are first seen.
pub struct PaletteNames<V> {
    names: RefCell<Names>,
    id: PhantomData<V>,
}

#[derive(Default)]
struct Names {
    ids: HashMap<Box<[u8]>, usize>,
    entries: BlockStateList,
    key: Vec<u8>,
}

impl<V> Default for PaletteNames<V> {
    fn default() -> Self {
        Self {
            names: RefCell::default(),
            id: PhantomData,
        }
    }
}

impl<V: PaletteId> PaletteNames<V> {
    pub fn new() -> Self {
        Self::default()
    }

    /// `None` once every id `V` can express is taken.
    pub fn intern<'p>(
        &self,
        name: &str,
        properties: impl IntoIterator<Item = (&'p str, &'p str)>,
    ) -> Option<V> {
        let names = &mut *self.names.borrow_mut();
        let properties: Vec<(&str, &str)> = properties.into_iter().collect();
        names.key.clear();
        for text in std::iter::once(name).chain(properties.iter().flat_map(|&(k, v)| [k, v])) {
            names
                .key
                .extend_from_slice(&(text.len() as u32).to_le_bytes());
            names.key.extend_from_slice(text.as_bytes());
        }
        if let Some(&index) = names.ids.get(names.key.as_slice()) {
            return V::from_index(index);
        }
        let index = names.entries.len();
        let id = V::from_index(index)?;
        names.ids.insert(names.key.as_slice().into(), index);
        names.entries.push(name, properties);
        Some(id)
    }

    /// The name and properties, in their original order, that `id` was handed
    /// out for.
    pub fn entry(&self, id: V) -> Option<(String, Vec<(String, String)>)> {
        let names = self.names.borrow();
        let index = id.index();
        (index < names.entries.len()).then(|| {
            let properties = names.entries.properties(index);
            (
                names.entries.name(index).to_string(),
                properties
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            )
        })
    }

    pub fn len(&self) -> usize {
        self.names.borrow().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn list(&self, ids: &[V]) -> Result<BlockStateList, V> {
        let names = self.names.borrow();
        let mut list = BlockStateList::default();
        for &id in ids {
            let index = id.index();
            if index >= names.entries.len() {
                return Err(id);
            }
            list.push(
                names.entries.name(index),
                names.entries.properties(index).iter(),
            );
        }
        Ok(list)
    }
}

impl<V: PaletteId> PaletteLookup<V> for PaletteNames<V> {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<V> {
        self.intern(name, properties.iter())
    }
}

/// The chunk as NBT, uncompressed, at the current `DataVersion`.
// chisle: structures, block and fluid ticks, post-processing and blending data
// are not in `Chunk`, so a written chunk drops them and a vanilla server reads
// their absence; carrying them in `Chunk` lifts this.
pub fn write_chunk(
    chunk: &Chunk,
    blocks: &PaletteNames<VoxelId>,
    biomes: &PaletteNames<u8>,
) -> Result<Vec<u8>, ErrorKind> {
    let sections = chunk
        .sections
        .iter()
        .map(|section| {
            let y = section.y;
            Ok(RawSection {
                y,
                block_states: section
                    .block_states
                    .as_ref()
                    .map(|c| pack::<Blocks, _, _>(c, blocks, y, "block_states"))
                    .transpose()?,
                biomes: section
                    .biomes
                    .as_ref()
                    .map(|c| pack::<Biomes, _, _>(c, biomes, y, "biomes"))
                    .transpose()?,
                noise_biomes: section
                    .noise_biomes
                    .as_ref()
                    .map(|c| pack::<NoiseBiomes, _, _>(c, biomes, y, "noise_biomes"))
                    .transpose()?,
                block_light: section.block_light.as_ref().map(|l| RawLight(l.0.to_vec())),
                sky_light: section.sky_light.as_ref().map(|l| RawLight(l.0.to_vec())),
            })
        })
        .collect::<Result<_, ErrorKind>>()?;
    let raw = RawChunk {
        data_version: VERSION.world_version,
        x_pos: chunk.pos.x,
        z_pos: chunk.pos.z,
        y_pos: chunk.min_section_y,
        status: chunk.status,
        sections,
        heightmaps: chunk.heightmaps.clone(),
        is_light_on: chunk.is_light_on,
        block_entities: chunk.block_entities.clone(),
        inhabited_time: chunk.inhabited_time,
        last_update: chunk.last_update,
        retrogen: chunk.retrogen.clone(),
    };
    let mut out = Vec::new();
    mcrs_minecraft_nbt::to_bytes(&raw, &mut out)?;
    Ok(out)
}

/// The inverse of the reader's `unpack`: the width comes from the same
/// `storage_bits`, and a single-entry palette carries no `data`.
fn pack<K: SectionKind, V: PaletteId + Hash + Eq + Default, const DIM: usize>(
    container: &PalettedContainer<V, DIM>,
    names: &PaletteNames<V>,
    y: i8,
    field: &'static str,
) -> Result<RawPalettedContainer, ErrorKind> {
    const { assert!(DIM == K::SIZE) };
    let len = match container {
        PalettedContainer::Homogeneous(_) => 1,
        PalettedContainer::Heterogeneous(data) => data.palette.len(),
    };
    let bits = K::storage_bits(len);
    let (ids, data) = container.to_palette_and_packed_data(bits as u8);
    let palette = names.list(&ids).map_err(|id| ErrorKind::UnnamedPaletteId {
        y,
        field,
        id: id.index() as u16,
    })?;
    Ok(RawPalettedContainer {
        palette,
        data: (bits > 0).then_some(PackedData(data)),
    })
}
