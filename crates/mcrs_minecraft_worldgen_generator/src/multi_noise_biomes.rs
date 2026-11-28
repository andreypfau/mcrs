use std::sync::Arc;

use crate::biome_window::BiomeWindow;
use crate::ids::require_registry;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::climate::{ClimateCursor, ParameterList, ParameterPoint, TargetPoint};
use mcrs_minecraft_biome::parameter_list::{MultiNoiseBiomeSourceParameterList, Preset};
use mcrs_minecraft_biome::source::{MultiNoiseBiomeEntry, MultiNoiseBiomeSource};
use mcrs_minecraft_biome::{
    registry_keys::BIOME, registry_keys::MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST,
};
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_registries::{Diagnostic, Diagnostics, Registries};
use mcrs_minecraft_resource::{Id, NarrowError, ResourceLocation, ResourceLocationBuf};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("registry {registry} holds no entry named {name}")]
pub struct UnknownEntry {
    pub registry: ResourceLocationBuf,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BiomeTableError {
    #[error(transparent)]
    Unknown(#[from] UnknownEntry),
    #[error(transparent)]
    Narrow(#[from] NarrowError),
    #[error("{0}")]
    NoTable(String),
}

pub trait BiomeNames {
    fn biome_named(&self, name: ResourceLocation<'_>) -> Result<Id<Biome>, UnknownEntry>;
}

impl BiomeNames for mcrs_minecraft_registries::Registry<BiomeFile> {
    fn biome_named(&self, name: ResourceLocation<'_>) -> Result<Id<Biome>, UnknownEntry> {
        self.by_name(name)
            .and_then(|id| Id::checked(id.number(), self.len()))
            .ok_or_else(|| UnknownEntry {
                registry: self.key().into(),
                name: name.to_string(),
            })
    }
}

fn narrow_biome(id: Id<Biome>) -> Result<u8, NarrowError> {
    id.narrow().map_err(|error| NarrowError {
        registry: BIOME.location().to_string(),
        ..error
    })
}

pub struct BiomeGrid {
    pub volume: SampleGrid,
    pub ids: Vec<u8>,
}

impl BiomeGrid {
    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        self.ids[self.volume.index_unchecked(x, y, z)]
    }

    #[must_use]
    pub fn window(&self) -> BiomeWindow<'_> {
        BiomeWindow::new(self.volume, self.volume, 0, 0, &self.ids)
    }
}

pub struct MultiNoiseBiomeTable {
    table: ParameterList<u8>,
}

impl MultiNoiseBiomeTable {
    pub fn of_preset(
        preset: Preset,
        biomes: &impl BiomeNames,
    ) -> Result<MultiNoiseBiomeTable, BiomeTableError> {
        let mut failure = None;
        let table = preset.parameter_list().try_map_values(|name| {
            match biomes
                .biome_named(*name)
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
    pub fn biome_at(&self, target: TargetPoint) -> Option<u8> {
        self.table.find_value(target).copied()
    }

    #[inline]
    pub fn cursor(&self) -> ClimateCursor<'_, u8> {
        self.table.cursor()
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.len() == 0
    }
}

#[derive(Clone, Default)]
pub struct PresetBiomeTables {
    tables: Vec<Arc<MultiNoiseBiomeTable>>,
}

impl PresetBiomeTables {
    pub fn build<'a>(
        lists: impl IntoIterator<Item = (&'a ResourceLocationBuf, Preset)>,
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
        let lists = require_registry::<MultiNoiseBiomeSourceParameterList>(registries, diagnostics);
        let biomes = require_registry::<BiomeFile>(registries, diagnostics);
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
