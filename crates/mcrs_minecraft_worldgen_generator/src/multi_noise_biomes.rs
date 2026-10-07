use std::sync::Arc;

use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::climate::{ParameterList, ParameterPoint, TargetPoint};
use mcrs_minecraft_biome::keys::{BIOME, MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST};
use mcrs_minecraft_biome::parameter_list::{
    MultiNoiseBiomeSourceParameterList, ParameterLists, Preset,
};
use mcrs_minecraft_biome::source::{MultiNoiseBiomeEntry, MultiNoiseBiomeSource};
use mcrs_minecraft_registry::shared::Resolved;
use mcrs_minecraft_registry::{
    Entries, Id, LoadReport, NarrowError, Registry, RegistrySet, UnknownEntry,
};
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
        biomes: &Registry<Biome>,
    ) -> Result<MultiNoiseBiomeTable, BiomeTableError> {
        let mut failure = None;
        let table = preset.parameter_list().try_map_values(|name| {
            match biomes
                .require_by_name(name)
                .map_err(BiomeTableError::from)
                .and_then(|id| biomes.narrow::<u8>(id).map_err(BiomeTableError::from))
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

    pub fn from_entries(
        biomes: &Registry<Biome>,
        entries: &[MultiNoiseBiomeEntry],
    ) -> Result<MultiNoiseBiomeTable, BiomeTableError> {
        let values: Vec<(ParameterPoint, u8)> = entries
            .iter()
            .map(|entry| {
                Ok((
                    ParameterPoint::from(&entry.parameters),
                    biomes.narrow::<u8>(entry.biome)?,
                ))
            })
            .collect::<Result<_, NarrowError>>()?;
        if values.is_empty() {
            return Err(BiomeTableError::NoTable(
                "a multi-noise source lists no biomes".to_owned(),
            ));
        }
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

pub struct PresetBiomeTables {
    tables: Entries<MultiNoiseBiomeSourceParameterList, Arc<MultiNoiseBiomeTable>>,
    biomes: Registry<Biome>,
}

impl PresetBiomeTables {
    pub fn build(
        lists: &Registry<MultiNoiseBiomeSourceParameterList>,
        entries: &ParameterLists,
        biomes: &Registry<Biome>,
        report: &mut LoadReport,
    ) -> Option<Self> {
        let mut built: Vec<(Preset, Result<Arc<MultiNoiseBiomeTable>, BiomeTableError>)> =
            Vec::new();
        let mut tables = Vec::with_capacity(lists.len());
        let mut refused = false;
        for id in lists.ids() {
            let preset = entries[id].preset;
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
                    let name = lists.name(id).map_or("", |name| name.as_str());
                    report.missing(
                        MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
                        name,
                        format!("the preset {}: {error}", preset.name()),
                    );
                }
            }
        }
        if refused {
            return None;
        }
        match Entries::new(lists, tables) {
            Ok(tables) => Some(PresetBiomeTables {
                tables,
                biomes: biomes.clone(),
            }),
            Err(error) => {
                report.invalid_report(error);
                None
            }
        }
    }

    pub fn resolve(set: &RegistrySet, report: &mut LoadReport) -> Option<Resolved<Self>> {
        let lists = report.registry(set, MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
        let biomes = report.registry(set, BIOME);
        let (lists, biomes) = (lists?, biomes?);
        let Some(entries) = set.entries() else {
            report.invalid_report(format_args!(
                "the entries of {MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST} are absent from the loaded set"
            ));
            return None;
        };
        Self::build(&lists, &entries, &biomes, report).map(Resolved::new)
    }

    pub fn get(
        &self,
        list: Id<MultiNoiseBiomeSourceParameterList>,
    ) -> Option<&Arc<MultiNoiseBiomeTable>> {
        self.tables.get(list)
    }

    pub fn table_of(
        &self,
        source: &MultiNoiseBiomeSource,
    ) -> Result<Arc<MultiNoiseBiomeTable>, BiomeTableError> {
        match (&source.preset, &source.biomes) {
            (Some(list), _) => self.get(*list).cloned().ok_or_else(|| {
                BiomeTableError::NoTable(format!("no parameter list is numbered {}", list.index()))
            }),
            (None, Some(entries)) => {
                MultiNoiseBiomeTable::from_entries(&self.biomes, entries).map(Arc::new)
            }
            (None, None) => Err(BiomeTableError::NoTable(
                "a multi-noise source names neither biomes nor a preset".to_owned(),
            )),
        }
    }
}
