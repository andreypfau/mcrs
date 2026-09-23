use mcrs_minecraft_core::ColumnPos;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use bevy_math::IVec3;
use bytes::Buf;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_feature::placer::WorldStates;
use mcrs_minecraft_worldgen_testing::{dump_string, open_dump};

use super::structures::{frozen_shared, preset};
use super::{
    biome_index, biome_registry, block_tags, blocks, build_settings_router, corpus_climate,
};
use crate::base_height;
use crate::feature_program::Resolver;
use crate::features::possible_biomes;
use crate::heightmap::{HeightmapKinds, heightmap_predicates};
use crate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::structures::index::{BiomeLookup, EndBiomes, StructureIndex};
use crate::structures::live_sets;
use mcrs_minecraft_worldgen_structure::frozen::{DimensionStructureTables, StructureKind};
use mcrs_minecraft_worldgen_structure::site::site_implies_piece;

const MAGIC: &[u8; 8] = b"MCSITES1";

struct DumpCase {
    chunk: ColumnPos,
    site: Option<(IVec3, bool)>,
}

struct DumpStructure {
    id: String,
    set: String,
    cases: Vec<DumpCase>,
}

struct DumpProbe {
    x: i32,
    z: i32,
    surface: i32,
    floor: i32,
}

struct DumpSelection {
    set: String,
    cases: Vec<(ColumnPos, Option<String>)>,
}

struct DumpSeed {
    seed: i64,
    rings: Vec<(String, Vec<(i32, i32)>)>,
    sites: Vec<(String, Vec<DumpStructure>)>,
    heights: Vec<(String, Vec<DumpProbe>)>,
    hardcoded: Vec<(String, Vec<DumpStructure>)>,
    selection: Vec<(String, Vec<DumpSelection>)>,
}

fn read_sites(r: &mut bytes::Bytes) -> Vec<(String, Vec<DumpStructure>)> {
    (0..r.get_u32_le())
        .map(|_| {
            let dimension = dump_string(r);
            let structures = (0..r.get_u32_le())
                .map(|_| DumpStructure {
                    id: dump_string(r),
                    set: dump_string(r),
                    cases: (0..r.get_u32_le())
                        .map(|_| {
                            let chunk = (r.get_i32_le(), r.get_i32_le());
                            let site = (r.get_u8() == 1).then(|| {
                                let position =
                                    IVec3::new(r.get_i32_le(), r.get_i32_le(), r.get_i32_le());
                                (position, r.get_u8() == 1)
                            });
                            DumpCase {
                                chunk: chunk.into(),
                                site,
                            }
                        })
                        .collect(),
                })
                .collect();
            (dimension, structures)
        })
        .collect()
}

fn read_dump() -> Vec<DumpSeed> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/tests/fixtures/structure_sites.bin");
    let mut r = open_dump(&path, MAGIC);
    let seeds = (0..r.get_u32_le())
        .map(|_| DumpSeed {
            seed: r.get_i64_le(),
            rings: (0..r.get_u32_le())
                .map(|_| {
                    let id = dump_string(&mut r);
                    let positions = (0..r.get_u32_le())
                        .map(|_| (r.get_i32_le(), r.get_i32_le()))
                        .collect();
                    (id, positions)
                })
                .collect(),
            sites: read_sites(&mut r),
            heights: (0..r.get_u32_le())
                .map(|_| {
                    let dimension = dump_string(&mut r);
                    let probes = (0..r.get_u32_le())
                        .map(|_| DumpProbe {
                            x: r.get_i32_le(),
                            z: r.get_i32_le(),
                            surface: r.get_i32_le(),
                            floor: r.get_i32_le(),
                        })
                        .collect();
                    (dimension, probes)
                })
                .collect(),
            hardcoded: read_sites(&mut r),
            selection: (0..r.get_u32_le())
                .map(|_| {
                    let dimension = dump_string(&mut r);
                    let sets = (0..r.get_u32_le())
                        .map(|_| DumpSelection {
                            set: dump_string(&mut r),
                            cases: (0..r.get_u32_le())
                                .map(|_| {
                                    let chunk = ColumnPos::new(r.get_i32_le(), r.get_i32_le());
                                    let selected =
                                        Some(dump_string(&mut r)).filter(|id| !id.is_empty());
                                    (chunk, selected)
                                })
                                .collect(),
                        })
                        .collect();
                    (dimension, sets)
                })
                .collect(),
        })
        .collect();
    assert!(!r.has_remaining(), "trailing bytes in {}", path.display());
    seeds
}

pub(super) struct Dimension {
    pub(super) settings: &'static str,
    pub(super) source: BiomeSource,
    pub(super) accessor_min_y: i32,
    pub(super) accessor_height: i32,
}

pub(super) fn dimension(id: &str) -> Dimension {
    match id {
        "minecraft:overworld" => Dimension {
            settings: "overworld",
            source: preset("minecraft:overworld"),
            accessor_min_y: -64,
            accessor_height: 384,
        },
        "minecraft:the_nether" => Dimension {
            settings: "nether",
            source: preset("minecraft:nether"),
            accessor_min_y: 0,
            accessor_height: 256,
        },
        "minecraft:the_end" => Dimension {
            settings: "end",
            source: BiomeSource::TheEnd,
            accessor_min_y: 0,
            accessor_height: 256,
        },
        other => panic!("the dump names an unexpected dimension {other}"),
    }
}

pub(super) fn build_index(dimension: &Dimension, seed: i64) -> StructureIndex {
    let frozen = frozen_shared();
    let source = &dimension.source;
    let mut mask = FixedBitSet::with_capacity(biome_index().len() as usize);
    for id in possible_biomes(source, |_| None) {
        mask.insert(biome_index().get(id.as_str()).unwrap() as usize);
    }
    let tables = DimensionStructureTables {
        frozen: Arc::clone(frozen),
        live: live_sets(frozen, &mask),
    };
    let biomes = match source {
        BiomeSource::MultiNoise(multi) => BiomeLookup::MultiNoise(Arc::new(
            MultiNoiseBiomeTable::resolve(multi, |name| biome_index().get(name).map(|id| id as u8))
                .unwrap(),
        )),
        BiomeSource::TheEnd => {
            BiomeLookup::TheEnd(EndBiomes::resolve(|id| biome_index().get(id)).unwrap())
        }
        _ => unreachable!(),
    };
    StructureIndex::new(
        Arc::new(tables),
        seed,
        Arc::new(build_settings_router(dimension.settings, seed as u64)),
        biomes,
        Some(heightmap_predicates(blocks(), block_tags())),
        Arc::clone(world_states()),
        Arc::clone(corpus_climate()),
        dimension.accessor_min_y,
        dimension.accessor_height,
    )
}

fn world_states() -> &'static Arc<WorldStates> {
    static STATES: LazyLock<Arc<WorldStates>> = LazyLock::new(|| {
        let biomes = biome_registry(&["minecraft:plains"]);
        let resolver = Resolver::new(
            &blocks().0,
            None,
            None,
            &biomes,
            0,
            &[],
            &super::corpus_features().block_state_providers,
        )
        .expect("the corpus resolves");
        Arc::new(resolver.world)
    });
    &STATES
}

/// The strict profile is the oracle. The fast profile can flip a biome at a
/// climate near-tie, which shifts the reservoir sample by one cell and moves
/// that ring position anywhere inside its 57×57-quart search window; the
/// observed count is one of 640, and a broken ring function would move
/// positions out of the window, not merely within it.
const RING_DIVERGENCE_BUDGET: usize = if mcrs_minecraft_worldgen_density::FAST_PROFILE {
    1
} else {
    0
};
const RING_WINDOW_CHUNKS: i32 = 15;

#[test]
fn stronghold_rings_match_the_oracle() {
    let dump = read_dump();
    let frozen = frozen_shared();
    let mut diverged = Vec::new();
    for entry in &dump {
        let index = build_index(&dimension("minecraft:overworld"), entry.seed);
        assert_eq!(entry.rings.len(), 1);
        for (set_id, positions) in &entry.rings {
            let set = frozen.set_ids[&ResourceLocation::parse(set_id).unwrap()];
            let ours = index.rings(set).unwrap();
            assert_eq!(ours.len(), positions.len(), "seed {}: {set_id}", entry.seed);
            for (i, (&ColumnPos { x, z }, &(ox, oz))) in ours.iter().zip(positions).enumerate() {
                if (x, z) == (ox, oz) {
                    continue;
                }
                assert!(
                    (x - ox).abs() <= RING_WINDOW_CHUNKS && (z - oz).abs() <= RING_WINDOW_CHUNKS,
                    "seed {}: {set_id} #{i} ours ({x}, {z}) oracle ({ox}, {oz}) is outside the biome window",
                    entry.seed
                );
                diverged.push((entry.seed, i, (x, z), (ox, oz)));
            }
        }
    }
    assert!(
        diverged.len() <= RING_DIVERGENCE_BUDGET,
        "{} ring positions diverged, budget {RING_DIVERGENCE_BUDGET}: {diverged:?}",
        diverged.len()
    );
}

#[test]
fn structure_sites_match_the_oracle() {
    let dump = read_dump();
    assert_eq!(dump.len(), 5, "the dump lost seeds");
    let frozen = frozen_shared();
    let mut cases = 0;
    let mut present = 0;
    let mut biome_ok = 0;
    for entry in &dump {
        let seed = entry.seed;
        assert_eq!(entry.sites.len(), 2);
        for (dimension_id, structures) in &entry.sites {
            let index = build_index(&dimension(dimension_id), seed);
            for structure in structures {
                let set = frozen.set_ids[&ResourceLocation::parse(&structure.set).unwrap()];
                let id = frozen.structure_ids[&ResourceLocation::parse(&structure.id).unwrap()];
                assert_eq!(structure.cases.len(), 16, "{}: cases", structure.id);
                for case in &structure.cases {
                    let ColumnPos { x, z } = case.chunk;
                    let label = format!("seed {seed} {} at chunk ({x}, {z})", structure.id);
                    assert!(index.gate(set, case.chunk), "{label}: gate");
                    let site = index.site(case.chunk, id);
                    assert_eq!(site.is_some(), case.site.is_some(), "{label}: present");
                    cases += 1;
                    if let (Some(site), Some((position, ok))) = (site, case.site) {
                        assert_eq!(site.position, position, "{label}: position");
                        assert_eq!(site.biome_ok, ok, "{label}: biome");
                        present += 1;
                        biome_ok += ok as u32;
                    }
                }
            }
        }
    }
    assert_eq!((cases, present, biome_ok), (5 * 28 * 16, 295, 206));
}

#[test]
fn hardcoded_sites_match_the_oracle() {
    let dump = read_dump();
    let frozen = frozen_shared();
    let mut cases = 0;
    let mut present = 0;
    let mut biome_ok = 0;
    for entry in &dump {
        let seed = entry.seed;
        assert_eq!(entry.hardcoded.len(), 2);
        for (dimension_id, structures) in &entry.hardcoded {
            let index = build_index(&dimension(dimension_id), seed);
            for structure in structures {
                let id = frozen.structure_ids[&ResourceLocation::parse(&structure.id).unwrap()];
                assert!(!matches!(
                    frozen.structures[id.0 as usize].kind,
                    StructureKind::Jigsaw { .. } | StructureKind::Mineshaft { .. }
                ));
                assert_eq!(structure.cases.len(), 16, "{}: cases", structure.id);
                for case in &structure.cases {
                    let ColumnPos { x, z } = case.chunk;
                    let label = format!("seed {seed} {} at chunk ({x}, {z})", structure.id);
                    let site = index.site(case.chunk, id);
                    assert_eq!(site.is_some(), case.site.is_some(), "{label}: present");
                    cases += 1;
                    if let (Some(site), Some((position, ok))) = (site, case.site) {
                        assert_eq!(site.position, position, "{label}: position");
                        assert_eq!(site.biome_ok, ok, "{label}: biome");
                        present += 1;
                        biome_ok += ok as u32;
                    }
                }
            }
        }
    }
    assert_eq!((cases, present, biome_ok), (5 * 21 * 16, 866, 447));
}

/// Every set whose entries all have a site, which since the mineshaft's port
/// is every set the dump holds.
#[test]
fn set_selection_matches_the_oracle() {
    let dump = read_dump();
    let frozen = frozen_shared();
    let mut cases = 0;
    let mut selected = 0;
    let mut skipped = std::collections::BTreeSet::new();
    for entry in &dump {
        let seed = entry.seed;
        assert_eq!(entry.selection.len(), 2);
        for (dimension_id, sets) in &entry.selection {
            let index = build_index(&dimension(dimension_id), seed);
            for dumped in sets {
                let set = frozen.set_ids[&ResourceLocation::parse(&dumped.set).unwrap()];
                let unported = frozen.sets[set.0 as usize]
                    .entries
                    .iter()
                    .any(|(structure, _)| {
                        site_implies_piece(&frozen.structures[structure.0 as usize].kind).is_none()
                    });
                if unported {
                    skipped.insert(dumped.set.as_str());
                    continue;
                }
                assert_eq!(dumped.cases.len(), 16, "{}: cases", dumped.set);
                for (chunk, expected) in &dumped.cases {
                    let ColumnPos { x, z } = *chunk;
                    let label = format!("seed {seed} {} at chunk ({x}, {z})", dumped.set);
                    let ours = index
                        .selected(set, *chunk)
                        .map(|id| frozen.structures[id.0 as usize].id.to_string());
                    assert_eq!(ours, *expected, "{label}");
                    cases += 1;
                    selected += ours.is_some() as u32;
                }
            }
        }
    }
    assert!(skipped.is_empty(), "sets without a site: {skipped:?}");
    assert_eq!((cases, selected), (5 * 21 * 16, 660));
}

#[test]
fn base_heights_match_the_oracle() {
    let dump = read_dump();
    let predicates = heightmap_predicates(blocks(), block_tags());
    let mut ws = Workspace::new();
    for entry in &dump {
        assert_eq!(entry.heights.len(), 2);
        for (dimension_id, probes) in &entry.heights {
            let dimension = dimension(dimension_id);
            let router = build_settings_router(dimension.settings, entry.seed as u64);
            assert_eq!(probes.len(), 64);
            for probe in probes {
                let mut height = |kind| {
                    base_height(
                        &router,
                        &mut ws,
                        &predicates,
                        kind,
                        probe.x,
                        probe.z,
                        dimension.accessor_min_y,
                        dimension.accessor_height,
                    )
                };
                let label = format!(
                    "seed {} {dimension_id} ({}, {})",
                    entry.seed, probe.x, probe.z
                );
                assert_eq!(
                    height(HeightmapKinds::SURFACE),
                    probe.surface,
                    "{label}: WORLD_SURFACE_WG"
                );
                assert_eq!(
                    height(HeightmapKinds::SOLID),
                    probe.floor,
                    "{label}: OCEAN_FLOOR_WG"
                );
            }
        }
    }
}

#[test]
fn site_positions_ignore_the_top_sixteen_seed_bits() {
    let dump = read_dump();
    let frozen = frozen_shared();
    for entry in &dump {
        for (dimension_id, structures) in &entry.sites {
            let dimension = dimension(dimension_id);
            let index = build_index(&dimension, entry.seed);
            let flipped = build_index(&dimension, entry.seed ^ (0xFFFF << 48));
            let mut checked = 0;
            let hardcoded = entry
                .hardcoded
                .iter()
                .find(|(id, _)| id == dimension_id)
                .map(|(_, structures)| structures.as_slice())
                .unwrap_or_default();
            for structure in structures.iter().chain(hardcoded) {
                let id = frozen.structure_ids[&ResourceLocation::parse(&structure.id).unwrap()];
                let reads_a_height = match &frozen.structures[id.0 as usize].kind {
                    StructureKind::Jigsaw { config, .. } => {
                        config.project_start_to_heightmap.is_some()
                    }
                    StructureKind::Fortress | StructureKind::Stronghold => false,
                    _ => true,
                };
                if reads_a_height {
                    continue;
                }
                for case in structure.cases.iter().take(3) {
                    let label = format!("seed {} {} at {:?}", entry.seed, structure.id, case.chunk);
                    let expected = index.site(case.chunk, id).expect(&label).position;
                    let actual = flipped.site(case.chunk, id).expect(&label).position;
                    assert_eq!(actual, expected, "{label}");
                    checked += 1;
                }
            }
            assert!(
                checked > 0,
                "seed {} {dimension_id}: nothing checked",
                entry.seed
            );
        }
    }
}
