use mcrs_minecraft_core::ColumnPos;
use std::sync::Arc;

use bevy_math::IVec3;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_biome::parameter_list::Preset;
use mcrs_minecraft_core::ResourceLocation;

use super::structures::{frozen_shared, preset};
use super::{block_tags, blocks, build_settings_router, corpus_biomes, corpus_climate};
use crate::features::possible_biomes;
use crate::heightmap::heightmap_predicates;
use crate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::structures::index::{BiomeLookup, StructureIndex};
use crate::structures::live_sets;
use mcrs_minecraft_worldgen_structure::frozen::{DimensionStructureTables, SetId};
use mcrs_minecraft_worldgen_structure::locate::locate_pos;
use mcrs_minecraft_worldgen_structure::site::Stub;

const SEED: u64 = 12345;

fn overworld() -> StructureIndex {
    let frozen = frozen_shared();
    let source = preset("minecraft:overworld");
    let mut mask = FixedBitSet::with_capacity(corpus_biomes().len());
    for name in possible_biomes(
        &source,
        corpus_biomes(),
        &super::biome_tags(),
        &crate::tests::parameter_lists().1,
    ) {
        mask.insert(corpus_biomes().by_name(name.as_str()).unwrap().index());
    }
    let tables = DimensionStructureTables {
        frozen: Arc::clone(frozen),
        live: live_sets(frozen, &mask),
    };
    let biomes = MultiNoiseBiomeTable::of_preset(Preset::Overworld, corpus_biomes()).unwrap();
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
}

fn set(id: &str) -> SetId {
    frozen_shared().set_ids[&ResourceLocation::read(id).unwrap()]
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
fn the_index_finds_the_nearest_village_and_stronghold() {
    let index = overworld();
    the_nearest_village_cell_yields_a_start(&index);
    locate_answers_the_nearest_stronghold_ring_position(&index);
}

fn the_nearest_village_cell_yields_a_start(index: &StructureIndex) {
    let villages = set("minecraft:villages");
    let frozen = frozen_shared();
    let entries: Vec<_> = frozen.sets[usize::from(villages.0)]
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
    assert_eq!(index.selected(villages, chunk), Some(structure));

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

/// `/locate structure minecraft:stronghold` from the origin: the ring position
/// nearest by chunk centre at `y = 32` whose site passes its biome test.
fn locate_answers_the_nearest_stronghold_ring_position(index: &StructureIndex) {
    let frozen = frozen_shared();
    let strongholds = set("minecraft:strongholds");
    let stronghold = frozen.structure_ids[&ResourceLocation::read("minecraft:stronghold").unwrap()];
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
        locate_pos(&frozen.sets[usize::from(strongholds.0)].placement, expected)
    );
    assert!(index.gate(strongholds, expected));
    assert_eq!(index.selected(strongholds, expected), Some(stronghold));
    assert_eq!(
        index.site(expected, stronghold).unwrap().position,
        IVec3::new(expected.min_block_x(), 0, expected.min_block_z())
    );
}

mod exhaustive {
    use super::*;

    /// With the villages the only live set, nothing wider widens the scan. This
    /// village's adapted box reaches seven chunks from its start chunk, one past
    /// what `max_distance_from_center` and the margin alone would allow.
    #[test]
    fn every_column_a_village_crosses_finds_its_start() {
        let frozen = frozen_shared();
        let villages = set("minecraft:villages");
        let plains = corpus_biomes().by_name("minecraft:plains").unwrap();
        let mut mask = FixedBitSet::with_capacity(corpus_biomes().len());
        mask.insert(plains.index());
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
            BiomeLookup::Fixed(plains.number()),
            Some(heightmap_predicates(blocks(), block_tags())),
            Default::default(),
            Arc::clone(corpus_climate()),
            -64,
            384,
        );
        let village =
            frozen.structure_ids[&ResourceLocation::read("minecraft:village_plains").unwrap()];
        let chunk = ColumnPos::new(-31, 72);
        let (_, start) = index
            .starts_reaching(chunk)
            .into_iter()
            .find(|(from, start)| *from == chunk && start.structure == village)
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
}
