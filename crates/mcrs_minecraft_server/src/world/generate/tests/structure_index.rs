use std::sync::{Arc, LazyLock};

use fixedbitset::FixedBitSet;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_world::biome::source::BiomeSource;
use mcrs_minecraft_worldgen::structure::placement::SpreadPlacement;

use super::structures::{frozen_shared, preset};
use super::{biome_index, block_tags, blocks, build_settings_router};
use crate::world::generate::features::possible_biomes;
use crate::world::generate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::world::generate::structures::index::{BiomeLookup, StructureIndex};
use crate::world::generate::structures::{DimensionStructureTables, SetId, live_sets};
use crate::world::heightmap::heightmap_predicates;

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
        .filter(|&(x, z)| index.gate(villages, x, z))
        .find_map(|chunk| Some((chunk, index.selected(villages, chunk)?)))
        .expect("a village within 128 chunks of the origin");
    assert!(entries.contains(&structure));
    assert!(index.starts_present(villages, chunk, structure));

    let site = index.site(chunk, structure).unwrap();
    assert!(site.biome_ok);
    assert_eq!(index.site(chunk, structure).unwrap(), site);
    assert!((site.position.x - chunk.0 * 16).abs() <= 16);
    assert!((site.position.z - chunk.1 * 16).abs() <= 16);
    assert!(
        site.centre.bounds.min.y < site.position.y && site.position.y < site.centre.bounds.max.y
    );
    assert_eq!(site.centre.ground_level_delta, 1);
}

#[test]
fn a_random_spread_gate_is_the_placement_test() {
    let index = overworld();
    let villages = set("minecraft:villages");
    let placement =
        SpreadPlacement::of(&frozen_shared().sets[villages.0 as usize].placement).unwrap();
    let mut hits = 0;
    for (x, z) in outward().take(2000) {
        let direct = placement.is_structure_chunk(SEED as i64, x, z, |_, _| false);
        assert_eq!(index.gate(villages, x, z), direct, "({x}, {z})");
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
    let (x, z) = rings[0];
    assert!(index.gate(strongholds, x, z));
    assert_eq!(index.rings(set("minecraft:villages")), None);
}
