use std::sync::Arc;

use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::climate::{ParameterList, ParameterPoint, TargetPoint};
use mcrs_minecraft_biome::keys::{BIOME, MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST};
use mcrs_minecraft_biome::parameter_list::{MultiNoiseBiomeSourceParameterList, Preset};
use mcrs_minecraft_biome::source::{MultiNoiseBiomeEntry, MultiNoiseBiomeSource};
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_core::{Id, NarrowError, ResourceLocation};
use mcrs_minecraft_registries::require_registry;
use mcrs_minecraft_registries::{Diagnostic, Diagnostics, Registries, RegistryOf};
use mcrs_minecraft_registry::UnknownEntry;
use mcrs_minecraft_worldgen_noise::sample_grid::SampleGrid;

/// Why a biome source has no climate table: a biome the registry does not hold,
/// an id the palette's byte cannot store, or a source that names no biomes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BiomeTableError {
    #[error(transparent)]
    Unknown(#[from] UnknownEntry),
    #[error(transparent)]
    Narrow(#[from] NarrowError),
    #[error("{0}")]
    NoTable(String),
}

/// The biome registry as a table builder reads it: a name in, the id out.
pub trait BiomeNames {
    fn biome_named(&self, name: &str) -> Result<Id<Biome>, UnknownEntry>;
}

impl BiomeNames for mcrs_minecraft_registry::Registry<Biome> {
    fn biome_named(&self, name: &str) -> Result<Id<Biome>, UnknownEntry> {
        self.require_by_name(name)
    }
}

impl BiomeNames for mcrs_minecraft_registries::Registry<RegistryOf<BiomeFile>> {
    fn biome_named(&self, name: &str) -> Result<Id<Biome>, UnknownEntry> {
        ResourceLocation::read(name)
            .ok()
            .and_then(|name| self.by_name(&name))
            .and_then(|id| Id::checked(id.number(), self.len()))
            .ok_or_else(|| UnknownEntry {
                registry: self.key().clone(),
                name: name.to_owned(),
            })
    }
}

fn narrow_biome(id: Id<Biome>) -> Result<u8, NarrowError> {
    id.narrow().map_err(|error| NarrowError {
        registry: BIOME.location().to_string(),
        ..error
    })
}

/// Biome ids over the column's quart cells, widened by one cell in every
/// direction: the zoom picks between eight quart corners and reaches outside
/// the column on all three axes, so the answer must never come from a
/// neighbouring column's stored palette.
pub struct BiomeGrid {
    pub volume: SampleGrid,
    pub ids: Vec<u8>,
}

impl BiomeGrid {
    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        self.ids[self.volume.index_unchecked(x, y, z)]
    }
}

/// A biome source's climate table with each entry already reduced to the id
/// the palette stores.
///
/// Resolving at build time rather than per cell means a cell's lookup ends at
/// the id itself, with no name to hash on the way — and the table is the same
/// for every column of the dimension.
pub struct MultiNoiseBiomeTable {
    table: ParameterList<u8>,
}

impl MultiNoiseBiomeTable {
    /// A table is built only when every biome resolved and fits the palette's
    /// byte: a substituted or truncated id would alias a different biome
    /// everywhere, in the grid the surface stage folds over as much as in the
    /// palette the client is sent.
    pub fn of_preset(
        preset: Preset,
        biomes: &impl BiomeNames,
    ) -> Result<MultiNoiseBiomeTable, BiomeTableError> {
        let mut failure = None;
        let table = preset.parameter_list().try_map_values(|name| {
            match biomes
                .biome_named(name)
                .map_err(BiomeTableError::from)
                .and_then(|id| narrow_biome(id).map_err(BiomeTableError::from))
            {
                Ok(id) => Some(id),
                Err(error) => {
                    failure.get_or_insert(error);
                    None
                }
            }
        });
        table
            .map(|table| MultiNoiseBiomeTable { table })
            .ok_or_else(|| failure.expect("a table is refused for a reason"))
    }

    pub(crate) fn from_entries(
        entries: &[MultiNoiseBiomeEntry],
    ) -> Result<MultiNoiseBiomeTable, BiomeTableError> {
        let values: Vec<(ParameterPoint, u8)> = entries
            .iter()
            .map(|entry| {
                Ok((
                    ParameterPoint::from(&entry.parameters),
                    narrow_biome(entry.biome)?,
                ))
            })
            .collect::<Result<_, NarrowError>>()?;
        Ok(MultiNoiseBiomeTable {
            table: ParameterList::new(values),
        })
    }

    pub fn climate(&self) -> &ParameterList<u8> {
        &self.table
    }

    #[inline]
    pub fn biome_at(&self, target: TargetPoint) -> u8 {
        *self.table.find_value(target)
    }

    /// [`Self::biome_at`] over a run of neighbouring cells: `last` carries the
    /// previous answer into the next search.
    #[inline]
    pub fn biome_at_from(&self, target: TargetPoint, last: &mut Option<usize>) -> u8 {
        *self.table.find_value_from(target, last)
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.len() == 0
    }
}

#[derive(Default)]
pub struct PresetBiomeTables {
    tables: Vec<Arc<MultiNoiseBiomeTable>>,
}

impl PresetBiomeTables {
    /// `lists` holds the name and preset of each parameter list in id order.
    pub fn build<'a>(
        lists: impl IntoIterator<Item = (&'a ResourceLocation, Preset)>,
        biomes: &impl BiomeNames,
        diagnostics: &mut Diagnostics,
    ) -> Option<Self> {
        let mut built: Vec<(Preset, Result<Arc<MultiNoiseBiomeTable>, BiomeTableError>)> =
            Vec::new();
        let mut tables = Vec::new();
        let mut refused = false;
        for (name, preset) in lists {
            let slot = match built.iter().position(|(built, _)| *built == preset) {
                Some(slot) => slot,
                None => {
                    built.push((
                        preset,
                        MultiNoiseBiomeTable::of_preset(preset, biomes).map(Arc::new),
                    ));
                    built.len() - 1
                }
            };
            match &built[slot].1 {
                Ok(table) => tables.push(Arc::clone(table)),
                Err(error) => {
                    refused = true;
                    diagnostics.extend([Diagnostic::Entry {
                        registry: MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST.location().into(),
                        entry: name.clone(),
                        pack: None,
                        file: None,
                        field: None,
                        message: format!("the preset {}: {error}", preset.name()),
                    }]);
                }
            }
        }
        (!refused).then_some(PresetBiomeTables { tables })
    }

    pub fn resolve(registries: &Registries, diagnostics: &mut Diagnostics) -> Option<Self> {
        let lists = require_registry::<_, RegistryOf<MultiNoiseBiomeSourceParameterList>>(
            registries,
            MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
            diagnostics,
        );
        let biomes = require_registry::<_, RegistryOf<BiomeFile>>(registries, BIOME, diagnostics);
        let (lists, biomes) = (lists?, biomes?);
        Self::build(
            lists.iter().map(|(_, name, list)| (name, list.preset)),
            biomes,
            diagnostics,
        )
    }

    pub fn get(
        &self,
        list: Id<MultiNoiseBiomeSourceParameterList>,
    ) -> Option<&Arc<MultiNoiseBiomeTable>> {
        self.tables.get(list.index())
    }

    pub fn table_of(
        &self,
        source: &MultiNoiseBiomeSource,
    ) -> Result<Arc<MultiNoiseBiomeTable>, BiomeTableError> {
        match source {
            MultiNoiseBiomeSource::Preset(list) => self.get(*list).cloned().ok_or_else(|| {
                BiomeTableError::NoTable(format!("no parameter list is numbered {}", list.index()))
            }),
            MultiNoiseBiomeSource::Biomes(entries) => {
                MultiNoiseBiomeTable::from_entries(entries).map(Arc::new)
            }
        }
    }
}
