use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_voxel::{BlockPos, ColumnPos};
use std::collections::HashMap;
use std::sync::Arc;

use fixedbitset::FixedBitSet;
use glam::IVec3;
use mcrs_minecraft_biome::parameter_list::Preset;

use super::ladder::structure_dimension;
use super::structures::{frozen_shared, preset};
use super::{block_tags, blocks, build_settings_router, corpus_biomes, corpus_climate};
use crate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::structures::index::{BiomeLookup, MAX_STRUCTURE_DISTANCE, StartTile, StructureIndex};
use crate::structures::live_sets;
use mcrs_minecraft_heightmap::heightmap_predicates;
use mcrs_minecraft_worldgen_structure::frozen::{DimensionStructureTables, SetId};
use mcrs_minecraft_worldgen_structure::locate::locate_pos;
use mcrs_minecraft_worldgen_structure::piece::Start;
use mcrs_minecraft_worldgen_structure::site::Stub;

const SEED: u64 = 12345;

fn overworld() -> StructureIndex {
    let frozen = frozen_shared();
    let source = preset("minecraft:overworld");
    let mask = super::possible_biome_mask(&source);
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

fn plains_villages() -> StructureIndex {
    let frozen = frozen_shared();
    let villages = set("minecraft:villages");
    let plains = crate::tests::biome_id(crate::tests::test_registries(), "minecraft:plains");
    let mut mask = FixedBitSet::with_capacity(corpus_biomes().len());
    mask.insert(plains.index());
    let tables = DimensionStructureTables {
        frozen: Arc::clone(frozen),
        live: live_sets(frozen, &mask)
            .into_iter()
            .filter(|(set, _)| *set == villages)
            .collect(),
    };
    StructureIndex::new(
        Arc::new(tables),
        SEED as i64,
        Arc::new(build_settings_router("overworld", SEED)),
        BiomeLookup::Fixed(plains.number()),
        Some(heightmap_predicates(blocks(), block_tags())),
        Default::default(),
        Arc::clone(corpus_climate()),
        -64,
        384,
    )
}

fn set(id: &str) -> SetId {
    frozen_shared().set_ids[&ResourceLocationBuf::read(id).unwrap()]
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
    let stronghold =
        frozen.structure_ids[&ResourceLocationBuf::read("minecraft:stronghold").unwrap()];
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

#[test]
fn the_four_tiles_of_a_column_hold_every_chunk_within_eight() {
    let columns = [
        (0, 0),
        (-1, -1),
        (7, 8),
        (8, 8),
        (9, -9),
        (-24, 40),
        (-25, -8),
        (23, 24),
    ];
    for column in columns.map(ColumnPos::from) {
        let tiles = StartTile::reaching(column);
        for (i, tile) in tiles.iter().enumerate() {
            assert!(!tiles[..i].contains(tile), "{column:?}: {tile:?} twice");
        }
        for tile in tiles {
            assert!(
                column
                    .square(MAX_STRUCTURE_DISTANCE)
                    .any(|chunk| StartTile::containing(chunk) == tile),
                "{column:?}: {tile:?} holds no chunk of the square"
            );
        }
        for chunk in column.square(MAX_STRUCTURE_DISTANCE) {
            let tile = StartTile::containing(chunk);
            assert!(
                tiles.contains(&tile),
                "{column:?}: {chunk:?} in {tile:?} outside {tiles:?}"
            );
            assert!(
                tile.chunks().any(|c| c == chunk),
                "{tile:?} does not list {chunk:?}"
            );
        }
    }
    assert_eq!(
        StartTile::containing(ColumnPos::new(-1, 16)),
        StartTile { x: -1, z: 1 }
    );
    assert_eq!(StartTile { x: -1, z: 0 }.chunks().count(), 256);
}

/// `reaching` over `tile_starts` against `starts_reaching` for every column within eight of `centre`: (columns with two or more starts, columns whose starts come from two or more tiles, columns with at least one start).
fn assert_tiles_agree(index: &StructureIndex, centre: ColumnPos) -> (usize, usize, usize) {
    let columns: Vec<ColumnPos> = centre.square(MAX_STRUCTURE_DISTANCE).collect();
    let mut tiles: HashMap<StartTile, Vec<(ColumnPos, Start)>> = HashMap::new();
    for tile in columns
        .iter()
        .flat_map(|&column| StartTile::reaching(column))
    {
        tiles.entry(tile).or_insert_with(|| index.tile_starts(tile));
    }
    let (mut crowded, mut crossing, mut found) = (0, 0, 0);
    for column in columns {
        let new = index.reaching(column, |tile| tiles[&tile].as_slice());
        assert_eq!(new, index.starts_reaching(column), "{column:?}");
        assert_eq!(
            index.beard_from(column, &new),
            index.beard(column),
            "{column:?}"
        );
        crowded += usize::from(new.len() >= 2);
        found += usize::from(!new.is_empty());
        let first = new.first().map(|(chunk, _)| StartTile::containing(*chunk));
        crossing += usize::from(
            new.iter()
                .any(|(chunk, _)| Some(StartTile::containing(*chunk)) != first),
        );
    }
    (crowded, crossing, found)
}

#[test]
fn reaching_over_tiles_lists_the_starts_the_memo_lists_in_its_order() {
    let village = structure_dimension("minecraft:village_plains", "minecraft:plains");
    let fixed = assert_tiles_agree(village.ctx.structures.as_deref().unwrap(), village.centre);
    let index = overworld();
    let villages = set("minecraft:villages");
    let centre = outward()
        .map(ColumnPos::from)
        .find(|&chunk| index.gate(villages, chunk) && index.selected(villages, chunk).is_some())
        .expect("a village within 128 chunks of the origin");
    let noise = assert_tiles_agree(&index, centre);
    assert!(
        fixed.0 > 0,
        "no column of the fixed dimension saw two starts, so the order went untested"
    );
    assert!(
        fixed.1 > 0,
        "no column of the fixed dimension drew starts from two tiles"
    );
    assert!(
        noise.2 > 0,
        "no column of the multi-noise dimension had a start, so only empty lists were compared"
    );
}

#[test]
fn reaching_leaves_out_a_start_more_than_eight_chunks_away() {
    let index = plains_villages();
    let column = ColumnPos::new(-31, 72);
    let structure = frozen_shared().structure_ids
        [&ResourceLocationBuf::read("minecraft:village_plains").unwrap()];
    let covering = |chunk: ColumnPos| {
        (
            chunk,
            Start {
                structure,
                pieces: Vec::new(),
                bounds: BoundingBox {
                    min: BlockPos::new(column.min_block_x(), 0, column.min_block_z()),
                    max: BlockPos::new(chunk.min_block_x(), 0, column.min_block_z()),
                },
            },
        )
    };
    let near = covering(ColumnPos::new(column.x + 8, column.z));
    let far = covering(ColumnPos::new(column.x + 9, column.z));
    assert!(StartTile::reaching(column).contains(&StartTile::containing(far.0)));
    let mut tiles: HashMap<StartTile, Vec<(ColumnPos, Start)>> = HashMap::new();
    tiles
        .entry(StartTile::containing(near.0))
        .or_default()
        .push(near.clone());
    tiles
        .entry(StartTile::containing(far.0))
        .or_default()
        .push(far);
    assert_eq!(
        index.reaching(column, |tile| tiles
            .get(&tile)
            .map_or(&[][..], Vec::as_slice)),
        vec![near]
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
        let index = plains_villages();
        let village =
            frozen.structure_ids[&ResourceLocationBuf::read("minecraft:village_plains").unwrap()];
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
