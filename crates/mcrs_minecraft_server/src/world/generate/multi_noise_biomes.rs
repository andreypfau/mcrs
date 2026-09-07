use mcrs_minecraft_world::biome::climate::{ParameterList, ParameterPoint, TargetPoint};
use mcrs_minecraft_world::biome::overworld_preset::{
    nether_parameter_list, overworld_parameter_list,
};
use mcrs_minecraft_world::biome::source::MultiNoiseBiomeSource;
use mcrs_minecraft_worldgen::volume::Volume;

/// Biome ids over the column's quart cells, widened by one cell in every
/// direction: the zoom picks between eight quart corners and reaches outside
/// the column on all three axes, so the answer must never come from a
/// neighbouring column's stored palette.
pub struct BiomeGrid {
    pub volume: Volume,
    pub ids: Vec<u8>,
}

impl BiomeGrid {
    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        self.ids[self.volume.index_unchecked(x, y, z)]
    }
}

/// A biome source's climate table with each entry already reduced to the
/// network id the palette stores.
///
/// Resolving at build time rather than per cell means a cell's lookup ends at
/// the id itself, with no name to hash on the way — and the table is the same
/// for every column of the dimension.
pub struct MultiNoiseBiomeTable {
    table: ParameterList<u8>,
}

impl MultiNoiseBiomeTable {
    /// `id_of` answers what network id a biome carries, and `None` where it has
    /// none the palette can store. It is called once per distinct biome of the
    /// source, not once per cell.
    ///
    /// A table is built only when every biome resolved: a substituted id would
    /// alias a different biome everywhere, in the grid the surface stage folds
    /// over as much as in the palette the client is sent.
    pub fn resolve(
        source: &MultiNoiseBiomeSource,
        id_of: impl Fn(&str) -> Option<u8>,
    ) -> Option<MultiNoiseBiomeTable> {
        let values: Vec<(ParameterPoint, u8)> = match (&source.preset, &source.biomes) {
            (Some(preset), _) => {
                let named = match preset.as_str() {
                    "minecraft:overworld" => overworld_parameter_list(),
                    "minecraft:nether" => nether_parameter_list(),
                    _ => return None,
                };
                named
                    .values()
                    .iter()
                    .map(|(point, biome)| Some((*point, id_of(biome)?)))
                    .collect::<Option<_>>()?
            }
            (None, Some(entries)) => entries
                .iter()
                .map(|entry| {
                    Some((
                        ParameterPoint::from(&entry.parameters),
                        id_of(entry.location.as_str())?,
                    ))
                })
                .collect::<Option<_>>()?,
            (None, None) => return None,
        };

        if values.is_empty() {
            return None;
        }
        Some(MultiNoiseBiomeTable {
            table: ParameterList::new(values),
        })
    }

    #[inline]
    pub fn biome_at(&self, target: TargetPoint) -> u8 {
        *self.table.find_value(target)
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.len() == 0
    }
}
