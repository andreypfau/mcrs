use mcrs_minecraft_core::ColumnPos;
use std::sync::{Arc, LazyLock};

use bevy_math::IVec3;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_biome::source::BiomeSource;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_structure::placement::SpreadPlacement;

use super::structures::{frozen_shared, preset};
use super::{biome_index, block_tags, blocks, build_settings_router, corpus_climate};
use crate::features::possible_biomes;
use crate::heightmap::heightmap_predicates;
use crate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::structures::index::{BiomeLookup, StructureIndex};
use crate::structures::live_sets;
use mcrs_minecraft_worldgen_structure::frozen::{DimensionStructureTables, SetId};
use mcrs_minecraft_worldgen_structure::locate::locate_pos;
use mcrs_minecraft_worldgen_structure::site::Stub;

const SEED: u64 = 12345;

fn overworld() -> &'static StructureIndex {
    static INDEX: LazyLock<StructureIndex> = LazyLock::new(|| {
        let frozen = frozen_shared();
        let source = preset("minecraft:overworld");
        let mut mask = FixedBitSet::with_capacity(biome_index().len() as usize);
        for id in possible_biomes(&source, |_| None) {
            mask.insert(biome_index().get(id.as_str()).unwrap() as usize);
        }
        let tables = DimensionStructureTables {
            frozen: Arc::clone(frozen),
            live: live_sets(frozen, &mask),
        };
        let BiomeSource::MultiNoise(multi) = source else {
            unreachable!()
        };
        let biomes = MultiNoiseBiomeTable::resolve(&multi, |name| {
            biome_index().get(name).map(|id| id as u8)
        })
        .unwrap();
        StructureIndex::new(
            Arc::new(tables),
            SEED as i64,
            Arc::new(build_settings_router("overworld", SEED)),
            BiomeLookup::MultiNoise(Arc::new(biomes)),
            Some(heightmap_predicates(blocks(), block_tags())),
            Default::default(),
            Arc::clone(corpus_climate()),
            -64,
            384,
        )
    });
    &INDEX
}

fn set(id: &str) -> SetId {
    frozen_shared().set_ids[&ResourceLocation::parse(id).unwrap()]
}

fn outward() -> impl Iterator<Item = (i32, i32)> {
    (0..128i32).flat_map(|radius| {
        (-radius..=radius).flat_map(move |x| {
            (-radius..=radius)
                .filter(move |z| x.abs() == radius || z.abs() == radius)
                .map(move |z| (x, z))
        })
    })
}

#[test]
fn the_nearest_village_cell_yields_a_start() {
    let index = overworld();
    let villages = set("minecraft:villages");
    let frozen = frozen_shared();
    let entries: Vec<_> = frozen.sets[villages.0 as usize]
        .entries
        .iter()
        .map(|(structure, _)| *structure)
        .collect();

    let (chunk, structure) = outward()
        .map(ColumnPos::from)
        .filter(|&chunk| index.gate(villages, chunk))
        .find_map(|chunk| Some((chunk, index.selected(villages, chunk)?)))
        .expect("a village within 128 chunks of the origin");
    assert!(entries.contains(&structure));
    assert!(index.starts_present(villages, chunk, structure));

    let site = index.site(chunk, structure).unwrap();
    assert!(site.biome_ok);
    assert_eq!(index.site(chunk, structure).unwrap(), site);
    assert!((site.position.x - chunk.min_block_x()).abs() <= 16);
    assert!((site.position.z - chunk.min_block_z()).abs() <= 16);
    let Stub::Jigsaw { centre, .. } = &site.stub else {
        panic!("a village site is a jigsaw site");
    };
    assert!(centre.bounds.min.y < site.position.y && site.position.y < centre.bounds.max.y);
    assert_eq!(centre.ground_level_delta, 1);
}

#[test]
fn a_random_spread_gate_is_the_placement_test() {
    let index = overworld();
    let villages = set("minecraft:villages");
    let placement =
        SpreadPlacement::of(&frozen_shared().sets[villages.0 as usize].placement).unwrap();
    let mut hits = 0;
    for (x, z) in outward().take(2000) {
        let direct = placement.is_structure_chunk(SEED as i64, ColumnPos::new(x, z), |_| false);
        assert_eq!(
            index.gate(villages, ColumnPos::new(x, z)),
            direct,
            "({x}, {z})"
        );
        hits += direct as u32;
    }
    assert!(hits > 0);
}

#[test]
fn the_stronghold_rings_hold_every_position() {
    let index = overworld();
    let strongholds = set("minecraft:strongholds");
    let rings = index.rings(strongholds).unwrap();
    assert_eq!(rings.len(), 128);
    assert!(index.gate(strongholds, rings[0]));
    assert_eq!(index.rings(set("minecraft:villages")), None);
}

/// `/locate structure minecraft:stronghold` from the origin: the ring position
/// nearest by chunk centre at `y = 32` whose site passes its biome test.
#[test]
fn locate_answers_the_nearest_stronghold_ring_position() {
    let index = overworld();
    let frozen = frozen_shared();
    let strongholds = set("minecraft:strongholds");
    let stronghold =
        frozen.structure_ids[&ResourceLocation::parse("minecraft:stronghold").unwrap()];
    let origin = IVec3::new(0, 64, 0);
    let (found, structure) = index
        .locate(origin, &[stronghold])
        .expect("a stronghold ring position passes its biome test");
    assert_eq!(structure, stronghold);

    let distance = |chunk: &ColumnPos| {
        let centre = IVec3::new(chunk.middle_block_x(), 32, chunk.middle_block_z());
        centre.as_dvec3().distance_squared(origin.as_dvec3())
    };
    let expected = index
        .rings(strongholds)
        .unwrap()
        .iter()
        .filter(|&&chunk| {
            index
                .site(chunk, stronghold)
                .is_some_and(|site| site.biome_ok)
        })
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
        .copied()
        .unwrap();
    assert_eq!(
        found,
        locate_pos(&frozen.sets[strongholds.0 as usize].placement, expected)
    );
    assert!(index.starts_present(strongholds, expected, stronghold));
    assert_eq!(
        index.site(expected, stronghold).unwrap().position,
        IVec3::new(expected.min_block_x(), 0, expected.min_block_z())
    );
}

/// With the villages the only live set, nothing wider widens the scan. This
/// village's adapted box reaches seven chunks from its start chunk, one past
/// what `max_distance_from_center` and the margin alone would allow.
#[test]
fn every_column_a_village_crosses_finds_its_start() {
    let frozen = frozen_shared();
    let villages = set("minecraft:villages");
    let plains = biome_index().get("minecraft:plains").unwrap();
    let mut mask = FixedBitSet::with_capacity(biome_index().len() as usize);
    mask.insert(plains as usize);
    let tables = DimensionStructureTables {
        frozen: Arc::clone(frozen),
        live: live_sets(frozen, &mask)
            .into_iter()
            .filter(|(set, _)| *set == villages)
            .collect(),
    };
    let index = StructureIndex::new(
        Arc::new(tables),
        SEED as i64,
        Arc::new(build_settings_router("overworld", SEED)),
        BiomeLookup::Fixed(plains),
        Some(heightmap_predicates(blocks(), block_tags())),
        Default::default(),
        Arc::clone(corpus_climate()),
        -64,
        384,
    );
    let village =
        frozen.structure_ids[&ResourceLocation::parse("minecraft:village_plains").unwrap()];
    let chunk = ColumnPos::new(-31, 72);
    let start = index
        .starts_at(chunk)
        .into_iter()
        .find(|start| start.structure == village)
        .expect("a plains village starts in the chunk");

    let bounds = start.bounds;
    let farthest = [
        (bounds.min.x >> 4) - chunk.x,
        (bounds.max.x >> 4) - chunk.x,
        (bounds.min.z >> 4) - chunk.z,
        (bounds.max.z >> 4) - chunk.z,
    ]
    .into_iter()
    .map(i32::abs)
    .max()
    .unwrap();
    assert_eq!(
        farthest, 7,
        "the village no longer reaches seven chunks out"
    );
    for x in bounds.min.x >> 4..=bounds.max.x >> 4 {
        for z in bounds.min.z >> 4..=bounds.max.z >> 4 {
            let column = ColumnPos::new(x, z);
            assert!(
                index
                    .starts_reaching(column)
                    .iter()
                    .any(|(from, found)| *from == chunk && found.structure == village),
                "{column:?} is crossed by the village started in {chunk:?} and does not find it"
            );
        }
    }
}
