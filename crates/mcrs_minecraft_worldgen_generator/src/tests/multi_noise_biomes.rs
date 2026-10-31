use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use mcrs_minecraft_biome::climate::{ClimateParameters, TargetPoint};
use mcrs_minecraft_biome::overworld_preset::overworld_parameter_list;
use mcrs_minecraft_biome::parameter_list::{MultiNoiseBiomeSourceParameterList, Preset};
use mcrs_minecraft_biome::source::MultiNoiseBiomeSource;
use mcrs_minecraft_biome::zoom::{obfuscate_seed, quart_cell};
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::{BlockPos, ResourceLocation};
use mcrs_minecraft_registries::Diagnostics;
use mcrs_minecraft_worldgen_density::program::Workspace;

use super::build_settings_router;
use crate::modern_carvers::climate_target_at;
use crate::multi_noise_biomes::{
    BiomeNames, BiomeTableError, MultiNoiseBiomeTable, PresetBiomeTables, UnknownEntry,
};
use crate::{multi_noise_grid, multi_noise_palettes};
use glam::IVec3;
use mcrs_minecraft_biome::Biome;

/// The preset's biomes numbered in the order the preset names them, which is
/// all a palette needs of a registry: distinct ids that round-trip.
pub(super) fn preset_ids() -> HashMap<String, u8> {
    let mut ids = HashMap::new();
    for (_, biome) in overworld_parameter_list().values() {
        let next = ids.len() as u8;
        ids.entry((*biome).to_owned()).or_insert(next);
    }
    ids
}

/// Biome names numbered in the order given.
struct NameList(Vec<String>);

impl NameList {
    fn of<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        NameList(names.into_iter().map(str::to_owned).collect())
    }
}

impl BiomeNames for NameList {
    fn biome_named(&self, name: &str) -> Result<Id<Biome>, UnknownEntry> {
        self.0
            .iter()
            .position(|held| held == name)
            .and_then(|number| Id::checked(u16::try_from(number).ok()?, self.0.len()))
            .ok_or_else(|| UnknownEntry {
                registry: mcrs_minecraft_biome::keys::BIOME.location().into(),
                name: name.to_owned(),
            })
    }
}

/// A registry numbering `ids` the way the map does.
fn registry_numbered_as(ids: &HashMap<String, u8>) -> NameList {
    let mut named: Vec<(&u8, &String)> = ids.iter().map(|(name, id)| (id, name)).collect();
    named.sort();
    NameList::of(named.into_iter().map(|(_, name)| name.as_str()))
}

pub(super) fn overworld_table() -> (MultiNoiseBiomeTable, HashMap<String, u8>) {
    let ids = preset_ids();
    let table = MultiNoiseBiomeTable::of_preset(Preset::Overworld, &registry_numbered_as(&ids))
        .expect("the overworld preset resolves");
    (table, ids)
}

fn y_sections() -> Vec<i32> {
    (-4..20).collect()
}

#[test]
#[expect(
    clippy::excessive_nesting,
    reason = "folds every step of the six climate parameters into a digest of the table's lookups, one loop per parameter, so a changed lookup shows"
)]
fn the_overworld_parameter_list_builds_the_overworld_table() {
    const STEPS: [f32; 5] = [-1.0, -0.5, 0.0, 0.5, 1.0];
    const DEPTHS: [f32; 5] = [-0.5, 0.0, 0.2, 0.55, 1.0];
    let (table, _) = overworld_table();
    assert_eq!(table.len(), overworld_parameter_list().len());

    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for temperature in STEPS {
        for humidity in STEPS {
            for continentalness in STEPS {
                for erosion in STEPS {
                    for depth in DEPTHS {
                        for weirdness in STEPS {
                            let target = TargetPoint::new(
                                temperature,
                                humidity,
                                continentalness,
                                erosion,
                                depth,
                                weirdness,
                            );
                            digest ^= u64::from(table.biome_at(target).unwrap());
                            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
                        }
                    }
                }
            }
        }
    }
    assert_eq!(digest, 15_074_558_872_385_058_528);
}

fn a_column_carries_its_cave_biome_under_its_surface_biome(
    router: &mcrs_minecraft_worldgen_density::router::NoiseRouter,
    table: &MultiNoiseBiomeTable,
    ids: &HashMap<String, u8>,
) {
    let sections = y_sections();

    let palettes = multi_noise_palettes(router, table, 0, 0, &sections);
    let grid = multi_noise_grid(router, table, 0, 0, &sections)
        .expect("the multi-noise path builds a grid");
    let first = sections[0];
    let column: Vec<u8> = sections
        .iter()
        .map(|&section_y| grid.get(1, (section_y - first) * 4, 1))
        .collect();
    let distinct: std::collections::BTreeSet<u8> = column.iter().copied().collect();
    let mut stored = std::collections::BTreeSet::new();
    for palette in &palettes {
        palette.for_each_distinct(|biome| {
            stored.insert(biome);
        });
    }

    let name_of = |biome: &str| *ids.get(biome).expect("the preset names it");
    assert_eq!(
        distinct,
        [
            name_of("minecraft:forest"),
            name_of("minecraft:dripstone_caves")
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<u8>>(),
        "the column should hold a surface biome over a cave biome"
    );
    assert!(
        stored.contains(&name_of("minecraft:forest"))
            && stored.contains(&name_of("minecraft:dripstone_caves")),
        "the stored column should hold both: {stored:?}"
    );
}

/// A source that lists its biomes rather than naming a preset.
#[test]
fn an_explicit_entry_list_resolves_to_the_registry_ids() {
    use mcrs_minecraft_biome::climate::ParameterRange;

    let flat = |value: f64| ClimateParameters {
        temperature: ParameterRange::Point(value),
        humidity: ParameterRange::Point(0.0),
        continentalness: ParameterRange::Point(0.0),
        erosion: ParameterRange::Point(0.0),
        depth: ParameterRange::Point(0.0),
        weirdness: ParameterRange::Point(0.0),
        offset: 0.0,
    };
    let registry =
        super::biome_registry(&["minecraft:swamp", "minecraft:plains", "minecraft:desert"]);
    let plains = super::biome_id(&registry, "minecraft:plains");
    let desert = super::biome_id(&registry, "minecraft:desert");
    let entries = [entry(flat(-1.0), plains), entry(flat(1.0), desert)];
    let table = MultiNoiseBiomeTable::from_entries(&entries).expect("an explicit list resolves");
    assert_eq!(table.len(), 2);
    assert_eq!(
        table.biome_at(mcrs_minecraft_biome::climate::TargetPoint::new(
            -1.0, 0.0, 0.0, 0.0, 0.0, 0.0
        )),
        Some(plains.narrow::<u8>().unwrap())
    );
    assert_eq!(
        table.biome_at(mcrs_minecraft_biome::climate::TargetPoint::new(
            1.0, 0.0, 0.0, 0.0, 0.0, 0.0
        )),
        Some(desert.narrow::<u8>().unwrap())
    );
}

fn entry(
    parameters: ClimateParameters,
    biome: Id<Biome>,
) -> mcrs_minecraft_biome::source::MultiNoiseBiomeEntry {
    mcrs_minecraft_biome::source::MultiNoiseBiomeEntry { parameters, biome }
}

/// The palette stores a biome in a byte. A registry that grew past 256 entries
/// would narrow an id into a different biome's, which nothing downstream can
/// see, so the table refuses to be built at all.
#[test]
fn a_biome_whose_id_does_not_fit_a_byte_refuses_the_table() {
    let mut names: Vec<String> = (0..256).map(|id| format!("a:filler_{id:03}")).collect();
    names.extend(preset_ids().into_keys());
    names.sort_unstable();
    let registry = NameList(names);

    let error = MultiNoiseBiomeTable::of_preset(Preset::Overworld, &registry)
        .err()
        .expect("every preset biome sorts past the 256 fillers");
    assert!(matches!(error, BiomeTableError::Narrow(_)), "{error}");
}

/// An id past the byte's range is refused, and the message names the registry
/// it is an id of.
#[test]
fn a_biome_id_beyond_the_narrow_width_is_refused() {
    use mcrs_minecraft_biome::climate::ParameterRange;

    let last = Id::checked(256, 257).expect("the 257th biome");

    let point = ParameterRange::Point(0.0);
    let entries = [entry(
        ClimateParameters {
            temperature: point.clone(),
            humidity: point.clone(),
            continentalness: point.clone(),
            erosion: point.clone(),
            depth: point.clone(),
            weirdness: point,
            offset: 0.0,
        },
        last,
    )];
    let error = MultiNoiseBiomeTable::from_entries(&entries)
        .err()
        .expect("an id of 256 does not fit a byte");
    assert!(matches!(error, BiomeTableError::Narrow(_)), "{error}");
    assert!(
        error.to_string().contains("minecraft:worldgen/biome"),
        "{error}"
    );
}

/// A fixed or Beta source is narrowed to the palette byte once, when its
/// column biomes are built, rather than degrading per column.
#[test]
fn a_fixed_or_beta_biome_beyond_the_narrow_width_is_refused_once() {
    use crate::ColumnBiomes;
    use mcrs_minecraft_biome::source::{BiomeSource, build_beta_lookup_table};

    let (first, last) = (Id::checked(0, 257).unwrap(), Id::checked(256, 257).unwrap());
    let beta = |stray| BiomeSource::Beta {
        land_biomes: std::array::from_fn(|i| if i == 10 { stray } else { first }),
        lookup: Arc::new(build_beta_lookup_table()),
    };

    for source in [BiomeSource::Fixed { biome: last }, beta(last)] {
        assert!(
            ColumnBiomes::new(Some(&source), None).is_err(),
            "{source:?}"
        );
    }
    assert!(matches!(
        ColumnBiomes::new(Some(&BiomeSource::Fixed { biome: first }), None),
        Ok(ColumnBiomes::Fixed(0))
    ));
    assert!(matches!(
        ColumnBiomes::new(Some(&beta(first)), None),
        Ok(ColumnBiomes::Beta(_))
    ));
}

#[test]
fn table_of_picks_the_named_or_inline_table() {
    use mcrs_minecraft_biome::climate::ParameterRange;

    let tables = super::preset_tables();
    let overworld = super::parameter_list_id("minecraft:overworld");
    let named = MultiNoiseBiomeSource::Preset(overworld);
    let picked = tables.table_of(&named).expect("a held list has a table");
    assert!(Arc::ptr_eq(
        &picked,
        tables.get(overworld).expect("the overworld list is held")
    ));

    let at = |value: f64, name: &str| {
        let parameters = ClimateParameters {
            temperature: ParameterRange::Point(value),
            humidity: ParameterRange::Point(0.0),
            continentalness: ParameterRange::Point(0.0),
            erosion: ParameterRange::Point(0.0),
            depth: ParameterRange::Point(0.0),
            weirdness: ParameterRange::Point(0.0),
            offset: 0.0,
        };
        entry(parameters, super::biome_id(super::test_registries(), name))
    };
    let inline = MultiNoiseBiomeSource::Biomes(vec![
        at(-1.0, "minecraft:plains"),
        at(0.0, "minecraft:desert"),
        at(1.0, "minecraft:swamp"),
    ]);
    assert_eq!(tables.table_of(&inline).expect("an inline list").len(), 3);
}

#[test]
fn the_shipped_parameter_lists_resolve_into_preset_tables() {
    let tables = super::preset_tables();
    let lists = super::test_registries()
        .registry::<MultiNoiseBiomeSourceParameterList>()
        .expect("the loaded registries hold the parameter lists");
    assert!(!lists.is_empty());
    for (id, name, list) in lists.iter() {
        let id = Id::checked(id.number(), lists.len()).expect("an id of the registry");
        let table = tables.get(id).expect("every list has a resolved table");
        assert_eq!(table.len(), list.preset.parameter_list().len(), "{name}");
    }
}

#[test]
fn entries_naming_one_preset_share_one_table() {
    let pair = ["test:one", "test:two"].map(|name| ResourceLocation::read(name).unwrap());
    let mut diagnostics = Diagnostics::default();
    let shared = PresetBiomeTables::build(
        pair.iter().map(|name| (name, Preset::Overworld)),
        super::corpus_biomes(),
        &mut diagnostics,
    )
    .unwrap_or_else(|| panic!("{diagnostics}"));
    let (one, two) = (Id::checked(0, 2).unwrap(), Id::checked(1, 2).unwrap());
    assert!(Arc::ptr_eq(
        shared.get(one).unwrap(),
        shared.get(two).unwrap()
    ));
}

#[test]
fn a_preset_naming_a_missing_biome_fails_the_load() {
    use mcrs_minecraft_biome::overworld_preset::nether_parameter_list;

    let missing = overworld_parameter_list().values()[0].1;
    let named: BTreeSet<&str> = overworld_parameter_list()
        .values()
        .iter()
        .chain(nether_parameter_list().values())
        .map(|(_, biome)| *biome)
        .filter(|biome| *biome != missing)
        .collect();
    let biomes = NameList::of(named);
    let name = ResourceLocation::read("minecraft:overworld").expect("a vanilla name");

    let mut diagnostics = Diagnostics::default();
    let built = PresetBiomeTables::build([(&name, Preset::Overworld)], &biomes, &mut diagnostics);
    assert!(built.is_none());
    let text = diagnostics.to_string();
    let line = text
        .lines()
        .find(|line| line.contains("overworld") && line.contains(missing))
        .unwrap_or_else(|| panic!("no line names the parameter list and {missing}: {text}"));
    assert!(
        line.contains("minecraft:worldgen/multi_noise_biome_source_parameter_list"),
        "{line}"
    );
}

#[test]
fn a_parameter_list_the_loader_does_not_hold_is_not_resolved() {
    let held = super::parameter_lists().len();
    let source = MultiNoiseBiomeSource::Preset(
        Id::checked(u16::try_from(held).unwrap(), held + 1).expect("an id past the held lists"),
    );
    assert!(matches!(
        super::preset_tables().table_of(&source),
        Err(BiomeTableError::NoTable(_))
    ));
}

/// The zoom reads eight quart corners around a block and they reach outside the
/// column horizontally, so the grid carries a ring of cells the palette does
/// not store; vertically the grid is the column, and the palette comes from its
/// cells.
#[test]
fn the_grid_rings_the_column_by_one_quart_cell() {
    let router = build_settings_router("overworld", 2);
    let (table, ids) = overworld_table();
    a_column_carries_its_cave_biome_under_its_surface_biome(&router, &table, &ids);
    let sections = y_sections();
    let (chunk_x, chunk_z) = (26, 90);
    let first = sections[0];

    let palettes = multi_noise_palettes(&router, &table, chunk_x * 16, chunk_z * 16, &sections);
    let grid = multi_noise_grid(&router, &table, chunk_x * 16, chunk_z * 16, &sections)
        .expect("the multi-noise path builds a grid");
    assert_eq!(
        grid.volume.size(),
        IVec3::new(6, sections.len() as i32 * 4, 6)
    );
    assert_eq!(
        grid.volume.min_block(),
        IVec3::new(chunk_x * 16 - 4, first * 16, chunk_z * 16 - 4)
    );
    assert!(
        palettes.iter().any(|palette| {
            let mut distinct = 0;
            palette.for_each_distinct(|_| distinct += 1);
            distinct > 1
        }),
        "a column with a section of several biomes"
    );

    let zoom_seed = obfuscate_seed(router.world_seed as i64);
    let min = grid.volume.min_block();
    let origin = IVec3::new(min.x >> 2, min.y >> 2, min.z >> 2);
    let (first_row, last_row) = (first * 4, sections[sections.len() - 1] * 4 + 3);
    let plain_pick = |block: BlockPos| {
        let quart = quart_cell(zoom_seed, block);
        let row = quart.y.clamp(first_row, last_row) - origin.y;
        grid.get(quart.x - origin.x, row, quart.z - origin.z)
    };
    for (index, &section_y) in sections.iter().enumerate() {
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let block = BlockPos::new(
                        chunk_x * 16 + x as i32,
                        section_y * 16 + y as i32,
                        chunk_z * 16 + z as i32,
                    );
                    assert_eq!(
                        palettes[index].get_cell(x, y, z).unwrap(),
                        plain_pick(block),
                        "block {x},{y},{z} of section {section_y}"
                    );
                }
            }
        }
    }

    let mut ws = Workspace::new();
    for (gx, gy, gz) in [(0, 0, 0), (5, grid.volume.size().y - 1, 5), (2, 7, 5)] {
        let target = climate_target_at(
            &router,
            &mut ws,
            grid.volume.block_x(gx) >> 2,
            grid.volume.block_y(gy) >> 2,
            grid.volume.block_z(gz) >> 2,
        );
        assert_eq!(
            Some(grid.get(gx, gy, gz)),
            table.biome_at(target),
            "grid cell {gx},{gy},{gz}"
        );
    }
}
