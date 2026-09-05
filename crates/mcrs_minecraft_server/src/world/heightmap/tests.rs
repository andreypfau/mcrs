use super::*;
use bevy_app::TaskPoolPlugin;
use bevy_asset::{AssetPlugin, AssetServer};
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::tag::TagLoader;
use mcrs_minecraft_world::block::definition::load_block_definitions;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::OnceLock;

fn workspace_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn corpus() -> &'static Blocks {
    static CORPUS: OnceLock<Blocks> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            file_path: workspace_root()
                .join("assets")
                .to_string_lossy()
                .into_owned(),
            ..Default::default()
        });
        let asset_server = app.world().resource::<AssetServer>().clone();
        let (definitions, _) =
            load_block_definitions(&asset_server).expect("the block definition corpus loads");
        Blocks(Arc::new(definitions))
    })
}

/// Expands one block tag file from the corpus on disk, following `#` references.
fn resolve_tag(name: &str, blocks: &Blocks, out: &mut HashSet<u32>) {
    let path = workspace_root()
        .join("assets/minecraft/tags/block")
        .join(format!("{}.json", name.trim_start_matches("minecraft:")));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let json: serde_json::Value = serde_json::from_str(&text).expect("valid tag file");
    for value in json["values"].as_array().expect("values array") {
        let id = match value {
            serde_json::Value::String(s) => s.as_str(),
            serde_json::Value::Object(o) => o["id"].as_str().expect("entry id"),
            _ => continue,
        };
        match id.strip_prefix('#') {
            Some(nested) => resolve_tag(nested, blocks, out),
            None => {
                if let Some(index) = blocks.index_of(id) {
                    out.insert(index);
                }
            }
        }
    }
}

fn tag_members(name: &'static str, blocks: &Blocks) -> HashSet<u32> {
    let mut ids = HashSet::new();
    resolve_tag(name, blocks, &mut ids);
    ids
}

fn tags(blocks: &Blocks) -> DynTagRegistry<Block> {
    let mut loader = TagLoader::<Block, u32>::new(&[]);
    for name in [
        "minecraft:blocks_motion_in_heightmap",
        "minecraft:blocks_motion_in_heightmap_no_leaves",
        "minecraft:leaves",
    ] {
        let location = ResourceLocation::parse(name)
            .expect("valid location")
            .to_arc();
        loader.insert(location, tag_members(name, blocks));
    }
    loader.freeze(blocks)
}

fn predicates() -> &'static HeightmapPredicates {
    static TABLE: OnceLock<HeightmapPredicates> = OnceLock::new();
    TABLE.get_or_init(|| heightmap_predicates(corpus(), &tags(corpus())))
}

fn state_of(block: &str) -> VoxelId {
    corpus()
        .block(block)
        .unwrap_or_else(|| panic!("the corpus declares {block}"))
        .default_state_id
        .into()
}

#[test]
fn air_never_blocks_motion() {
    let blocks = corpus();
    let table = predicates();
    for index in 0..blocks.state_count() {
        let id = VoxelId(index as u16);
        if blocks
            .state(id.into())
            .flags
            .contains(BlockStateFlags::IS_AIR)
        {
            assert_eq!(
                table.get(id),
                HeightmapKinds::empty(),
                "air state {index} satisfies a heightmap predicate"
            );
        }
    }
}

#[test]
fn a_fluid_bearing_state_is_never_air() {
    let blocks = corpus();
    let mut fluid_states = 0usize;
    for index in 0..blocks.state_count() {
        let state = blocks.state(BlockStateId(index as u16));
        if state.fluid.is_some() {
            fluid_states += 1;
            assert!(
                !state.flags.contains(BlockStateFlags::IS_AIR),
                "state {index} is air and holds a fluid"
            );
        }
    }
    assert!(fluid_states > 0, "the corpus declares no fluid state");
}

#[test]
fn blocks_motion_is_exactly_no_leaves_plus_leaves() {
    let blocks = corpus();
    let motion = tag_members("minecraft:blocks_motion_in_heightmap", blocks);
    let no_leaves = tag_members("minecraft:blocks_motion_in_heightmap_no_leaves", blocks);
    let leaves = tag_members("minecraft:leaves", blocks);

    assert!(!motion.is_empty() && !leaves.is_empty());
    let union: HashSet<u32> = no_leaves.union(&leaves).copied().collect();
    let only_in_motion: Vec<_> = motion.difference(&union).copied().collect();
    let only_in_union: Vec<_> = union.difference(&motion).copied().collect();
    assert!(
        only_in_motion.is_empty() && only_in_union.is_empty(),
        "blocks_motion != no_leaves u leaves: {} only in motion, {} only in the union",
        only_in_motion.len(),
        only_in_union.len()
    );
    assert!(motion.len() > no_leaves.len(), "leaves add nothing");
}

#[test]
fn the_tag_predicates_are_the_same_for_every_state_of_a_block() {
    let blocks = corpus();
    let table = predicates();
    for block in blocks.blocks() {
        let base = block.base_state_id.0;
        let solid = table.get(VoxelId(base)) & HeightmapKinds::SOLID;
        // `NO_LEAVES` also answers to the state's own fluid, so its tag half is
        // only comparable among the states that carry none.
        let dry = |id: VoxelId| blocks.state(id.into()).fluid.is_none();
        let no_leaves = (0..block.state_count)
            .map(|offset| VoxelId(base + offset))
            .find(|&id| dry(id))
            .map(|id| table.get(id) & HeightmapKinds::NO_LEAVES);
        for offset in 0..block.state_count {
            let id = VoxelId(base + offset);
            let name = block.identifier.as_str();
            assert_eq!(
                table.get(id) & HeightmapKinds::SOLID,
                solid,
                "{name} state {offset} disagrees with its block"
            );
            if dry(id) {
                assert_eq!(
                    Some(table.get(id) & HeightmapKinds::NO_LEAVES),
                    no_leaves,
                    "{name} state {offset} disagrees with its block"
                );
            }
        }
    }
}

#[test]
fn water_and_leaves_land_where_the_lattice_says() {
    let table = predicates();
    let water = table.get(state_of("minecraft:water"));
    assert_eq!(
        water,
        HeightmapKinds::SURFACE | HeightmapKinds::MOTION | HeightmapKinds::NO_LEAVES
    );

    let leaves = table.get(state_of("minecraft:oak_leaves"));
    assert_eq!(
        leaves,
        HeightmapKinds::SURFACE | HeightmapKinds::SOLID | HeightmapKinds::MOTION
    );

    assert_eq!(
        table.get(state_of("minecraft:stone")),
        HeightmapKinds::all()
    );
    assert_eq!(
        table.get(state_of("minecraft:glass")),
        HeightmapKinds::all()
    );
    assert_eq!(
        table.get(state_of("minecraft:air")),
        HeightmapKinds::empty()
    );
    assert_eq!(
        table.get(state_of("minecraft:short_grass")),
        HeightmapKinds::SURFACE
    );
}

// ─── The fused pass against a naive per-map scan ─────────────────────────────

/// Scans each map on its own from the ceiling down, with no bounds and no
/// shared state. Slow and obviously right, so it can referee the fused pass.
fn naive_heightmaps(
    sections: &[Option<(BlockPalette, BiomePalette)>],
    y_sections: &[i32],
    table: &HeightmapPredicates,
) -> ColumnHeightmapSet {
    let first = *y_sections.first().unwrap();
    let last = *y_sections.last().unwrap();
    let min_y = first * 16;
    let mut set = ColumnHeightmapSet::new(((last - first + 1) * 16) as u32, min_y);
    for kind in [
        HeightmapKinds::SURFACE,
        HeightmapKinds::SOLID,
        HeightmapKinds::MOTION,
        HeightmapKinds::NO_LEAVES,
    ] {
        for z in 0..16usize {
            for x in 0..16usize {
                let mut found = min_y;
                for y in (min_y..(last + 1) * 16).rev() {
                    let index = (y.div_euclid(16) - first) as usize;
                    let Some((blocks, _)) = sections[index].as_ref() else {
                        continue;
                    };
                    let id = blocks.0.get(x, y.rem_euclid(16) as usize, z);
                    if table.get(id).contains(kind) {
                        found = y + 1;
                        break;
                    }
                }
                set.set(kind, x, z, found);
            }
        }
    }
    set
}

fn assert_same(built: &ColumnHeightmapSet, expected: &ColumnHeightmapSet, what: &str) {
    for (name, a, b) in [
        ("surface", &built.surface.0, &expected.surface.0),
        ("solid", &built.solid.0, &expected.solid.0),
        ("motion", &built.motion.0, &expected.motion.0),
        ("no_leaves", &built.no_leaves.0, &expected.no_leaves.0),
    ] {
        for z in 0..16usize {
            for x in 0..16usize {
                assert_eq!(a.get(x, z), b.get(x, z), "{what}: {name} at ({x}, {z})");
            }
        }
    }
}

/// Four sections of world, one column per interesting stack.
fn sample_column() -> (Vec<Option<(BlockPalette, BiomePalette)>>, Vec<i32>) {
    let y_sections = vec![0, 1, 2, 3];
    let mut sections: Vec<Option<(BlockPalette, BiomePalette)>> =
        vec![Some((BlockPalette::default(), BiomePalette::default())); 4];

    let stone = state_of("minecraft:stone");
    let sand = state_of("minecraft:sand");
    let water = state_of("minecraft:water");
    let log = state_of("minecraft:oak_log");
    let leaves = state_of("minecraft:oak_leaves");
    let glass = state_of("minecraft:glass");
    let grass = state_of("minecraft:short_grass");

    let put = |sections: &mut Vec<Option<(BlockPalette, BiomePalette)>>,
               x: usize,
               y: i32,
               z: usize,
               id: VoxelId| {
        let (blocks, _) = sections[(y / 16) as usize].as_mut().unwrap();
        blocks.set_cell(x, (y % 16) as usize, z, id);
    };

    for z in 0..16usize {
        for x in 0..16usize {
            for y in 0..20 {
                put(&mut sections, x, y, z, stone);
            }
        }
    }

    // (0, z): water over sand — SOLID at the floor, NO_LEAVES at the surface.
    for z in 0..16usize {
        put(&mut sections, 0, 20, z, sand);
        for y in 21..40 {
            put(&mut sections, 0, y, z, water);
        }
    }
    // (1, z): a canopy over a trunk — the other way round.
    for z in 0..16usize {
        for y in 20..28 {
            put(&mut sections, 1, y, z, log);
        }
        for y in 28..33 {
            put(&mut sections, 1, y, z, leaves);
        }
    }
    // (2, z): glass, which blocks motion but passes light.
    for z in 0..16usize {
        put(&mut sections, 2, 20, z, glass);
    }
    // (3, z): a plant on top — non-air, satisfying nothing else.
    for z in 0..16usize {
        put(&mut sections, 3, 20, z, grass);
    }
    // (4, z): open to the top of the world.
    for z in 0..16usize {
        for y in 0..20 {
            put(&mut sections, 4, y, z, stone);
        }
        put(&mut sections, 4, 63, z, stone);
    }
    // (5, z): nothing at all.
    for z in 0..16usize {
        for y in 0..20 {
            put(&mut sections, 5, y, z, VoxelId::default());
        }
    }

    (sections, y_sections)
}

#[test]
fn the_fused_pass_matches_the_naive_scan() {
    let table = predicates();
    let (sections, y_sections) = sample_column();
    let built = build_column_heightmaps(&sections, &y_sections, table).expect("a built column");
    let expected = naive_heightmaps(&sections, &y_sections, table);
    assert_same(&built, &expected, "sample column");

    // Spot-check the two cases where SOLID and NO_LEAVES are incomparable.
    assert_eq!(built.surface.0.get(0, 0), 40);
    assert_eq!(built.solid.0.get(0, 0), 21);
    assert_eq!(built.no_leaves.0.get(0, 0), 40);
    assert_eq!(built.motion.0.get(0, 0), 40);

    assert_eq!(built.surface.0.get(1, 0), 33);
    assert_eq!(built.solid.0.get(1, 0), 33);
    assert_eq!(built.no_leaves.0.get(1, 0), 28);
    assert_eq!(built.motion.0.get(1, 0), 33);

    assert_eq!(built.surface.0.get(3, 0), 21);
    assert_eq!(built.solid.0.get(3, 0), 20);
    assert_eq!(built.no_leaves.0.get(3, 0), 20);

    assert_eq!(built.surface.0.get(5, 0), 0);
    assert_eq!(built.solid.0.get(5, 0), 0);
}

#[test]
fn an_empty_column_reports_its_floor() {
    let table = predicates();
    let y_sections = vec![-4, -3];
    let sections: Vec<Option<(BlockPalette, BiomePalette)>> =
        vec![Some((BlockPalette::default(), BiomePalette::default())); 2];
    let built = build_column_heightmaps(&sections, &y_sections, table).expect("a built column");
    assert_eq!(built.surface.0.get(7, 9), -64);
    assert_eq!(built.motion.0.get(7, 9), -64);
}

// ─── Incremental maintenance against a rebuild ───────────────────────────────

fn spawn_column(
    sections: &[Option<(BlockPalette, BiomePalette)>],
    y_sections: &[i32],
) -> (App, Entity, Vec<Entity>) {
    use mcrs_voxel_world::world::storage::column::{ColumnIndex, ColumnSlot};

    let mut app = App::new();
    app.add_message::<BlockPlaced>();
    app.insert_resource(predicates().clone());
    app.add_systems(bevy_app::Update, update_column_heightmaps);

    let section_entities: Vec<Entity> = sections
        .iter()
        .map(|section| {
            let blocks = section.as_ref().map(|(b, _)| b.clone()).unwrap_or_default();
            app.world_mut().spawn(ChunkBlocks::new(blocks)).id()
        })
        .collect();

    let first = *y_sections.first().unwrap();
    let height = ((*y_sections.last().unwrap() - first + 1) * 16) as u32;
    let min_y = first * 16;
    let mut chunks = ColumnChunks::new(first, y_sections.len());
    for (&section_y, &entity) in y_sections.iter().zip(&section_entities) {
        chunks.set_loaded(section_y, entity);
    }
    let column = app
        .world_mut()
        .spawn((chunks, ColumnHeightmapSet::new(height, min_y)))
        .id();

    let mut index = ColumnIndex::default();
    index.insert(
        ColumnPos::new(0, 0),
        ColumnSlot {
            entity: column,
            section_count: y_sections.len() as u32,
        },
    );
    app.world_mut().spawn(index);

    (app, column, section_entities)
}

fn write_maps(app: &mut App, column: Entity, built: &ColumnHeightmapSet) {
    app.world_mut().entity_mut(column).insert(built.clone());
}

fn read_maps(app: &App, column: Entity) -> ColumnHeightmapSet {
    let entity = app.world().entity(column);
    ColumnHeightmapSet {
        surface: entity.get::<SurfaceHeightmap>().unwrap().clone(),
        solid: entity.get::<SolidHeightmap>().unwrap().clone(),
        motion: entity.get::<MotionHeightmap>().unwrap().clone(),
        no_leaves: entity.get::<NoLeavesHeightmap>().unwrap().clone(),
    }
}

#[test]
fn a_series_of_edits_stays_bit_for_bit_equal_to_a_rebuild() {
    use mcrs_minecraft_block::block::BlockUpdateFlags;
    use mcrs_voxel_math::{BlockPos, ChunkPos};

    let table = predicates();
    let (mut sections, y_sections) = sample_column();
    let (mut app, column, section_entities) = spawn_column(&sections, &y_sections);
    write_maps(
        &mut app,
        column,
        &build_column_heightmaps(&sections, &y_sections, table).unwrap(),
    );

    let air = VoxelId::default();
    let edits = [
        // Remove the very top of the water column: the expensive descent.
        (0usize, 39i32, 0usize, air),
        (0, 38, 0, air),
        // Remove the canopy top, then the trunk under it.
        (1, 32, 0, air),
        (1, 31, 0, air),
        // Take out the glass that was the whole column.
        (2, 20, 0, air),
        // Raise a column that had nothing.
        (5, 41, 0, state_of("minecraft:stone")),
        // Add a plant on top: SURFACE only.
        (3, 21, 0, state_of("minecraft:short_grass")),
        // Deep below the top: must change nothing.
        (4, 5, 0, air),
        // Waterlog the sand floor under the ocean.
        (0, 20, 0, state_of("minecraft:water")),
        // Swap the pillar's top for leaves: SOLID holds, NO_LEAVES falls far.
        (4, 63, 0, state_of("minecraft:oak_leaves")),
        // Then take the leaves away, dropping SOLID and MOTION together.
        (4, 63, 0, air),
    ];

    for (x, y, z, id) in edits {
        let index = (y / 16) as usize;
        let (blocks, _) = sections[index].as_mut().unwrap();
        let old = blocks.set_cell(x, (y % 16) as usize, z, id);
        app.world_mut()
            .entity_mut(section_entities[index])
            .get_mut::<ChunkBlocks>()
            .unwrap()
            .make_mut()
            .set_cell(x, (y % 16) as usize, z, id);
        app.world_mut().write_message(BlockPlaced {
            chunk: section_entities[index],
            chunk_pos: ChunkPos::new(0, y / 16, 0),
            block_pos: BlockPos::new(x as i32, y, z as i32),
            old_state: old,
            new_state: id,
            flags: BlockUpdateFlags::all(),
        });
        app.update();

        let rebuilt = build_column_heightmaps(&sections, &y_sections, table).unwrap();
        assert_same(
            &read_maps(&app, column),
            &rebuilt,
            &format!("after ({x}, {y}, {z})"),
        );
    }
}

#[test]
fn priming_merges_partial_ranges_with_max() {
    use mcrs_voxel_world::world::storage::column::{ColumnIndex, ColumnSlot};

    let mut app = App::new();
    app.init_resource::<PendingColumnHeightmaps>();
    app.add_systems(bevy_app::Update, prime_column_heightmaps);

    let dim = app
        .world_mut()
        .spawn((ColumnIndex::default(), DimensionTypeConfig::new(-64, 384)))
        .id();
    let column = app.world_mut().spawn(InDimension(dim)).id();
    app.world_mut().get_mut::<ColumnIndex>(dim).unwrap().insert(
        ColumnPos::new(2, -3),
        ColumnSlot {
            entity: column,
            section_count: 1,
        },
    );

    let mut upper = ColumnHeightmapSet::new(64, 64);
    upper.surface.0.set(1, 1, 90);
    // The upper range holds no solid block: its floor must not outrank the
    // lower range's real answer.
    upper.solid.0.set(1, 1, 64);
    let mut lower = ColumnHeightmapSet::new(64, 0);
    lower.surface.0.set(1, 1, 30);
    lower.solid.0.set(1, 1, 12);

    for built in [lower, upper] {
        app.world_mut()
            .resource_mut::<PendingColumnHeightmaps>()
            .0
            .insert(ColumnPos::new(2, -3), built);
        app.update();
    }

    let entity = app.world().entity(column);
    let surface = entity
        .get::<SurfaceHeightmap>()
        .expect("priming attached it");
    let solid = entity.get::<SolidHeightmap>().expect("priming attached it");
    // Sized to the dimension, not to either range.
    assert_eq!(surface.0.height(), 384);
    assert_eq!(surface.0.min_y(), -64);
    assert_eq!(surface.0.get(1, 1), 90);
    assert_eq!(solid.0.get(1, 1), 12);
    // Untouched columns keep the dimension floor.
    assert_eq!(surface.0.get(0, 0), -64);
    assert!(
        app.world()
            .resource::<PendingColumnHeightmaps>()
            .0
            .is_empty()
    );
}

#[test]
fn the_chunk_packet_carries_the_three_client_heightmaps() {
    use mcrs_minecraft_protocol::chunk::ChunkData;
    use mcrs_minecraft_protocol::{Decode, Encode};
    use std::borrow::Cow;

    let mut surface = SurfaceHeightmap(ColumnHeightmap::new(384, -64));
    let motion = MotionHeightmap(ColumnHeightmap::new(384, -64));
    let no_leaves = NoLeavesHeightmap(ColumnHeightmap::new(384, -64));
    surface.0.set(3, 5, 70);

    let maps = client_heightmaps(&surface, &motion, &no_leaves);
    assert_eq!(
        maps.iter().map(|(kind, _)| kind.0).collect::<Vec<_>>(),
        vec![1, 4, 5],
        "WORLD_SURFACE, MOTION_BLOCKING, MOTION_BLOCKING_NO_LEAVES"
    );
    // The length a client's SimpleBitStorage(ceillog2(384 + 1), 256) holds:
    // 9 bits per entry, 7 entries per long, 256 entries.
    for (_, data) in &maps {
        assert_eq!(data.len(), 37);
    }

    let mut bytes = Vec::new();
    ChunkData {
        heightmaps: maps
            .iter()
            .map(|(kind, data)| (*kind, Cow::Borrowed(data.as_slice())))
            .collect(),
        data: &[],
        block_entities: Cow::Borrowed(&[]),
    }
    .encode(&mut bytes)
    .expect("the chunk packet encodes");

    let decoded = ChunkData::decode(&mut bytes.as_slice()).expect("and decodes");
    assert_eq!(decoded.heightmaps.len(), 3);
    assert_eq!(decoded.heightmaps[0].0.0, 1);
    assert_eq!(&*decoded.heightmaps[0].1, surface.0.raw_longs());
    assert_eq!(surface.0.get(3, 5), 70);
}

// ─── Packed storage ──────────────────────────────────────────────────────────

#[test]
fn heightmap_new_dimensions_sized_correctly() {
    let h = ColumnHeightmap::new(384, 0);
    assert_eq!(h.storage().bits_per_entry(), 9);
    assert_eq!(h.storage().entry_count(), 256);
    // 256 entries / (64 / 9 = 7 per long) = 37 longs.
    assert_eq!(h.raw_longs().len(), 37);
}

#[test]
fn heightmap_set_get_round_trip() {
    let mut h = ColumnHeightmap::new(384, -64);
    for z in 0..BLOCKS::SIZE {
        for x in 0..BLOCKS::SIZE {
            let y = (z * BLOCKS::SIZE + x) as i32 - 64;
            h.set(x, z, y);
        }
    }
    for z in 0..BLOCKS::SIZE {
        for x in 0..BLOCKS::SIZE {
            let y = (z * BLOCKS::SIZE + x) as i32 - 64;
            assert_eq!(h.get(x, z), y, "scalar mismatch at ({x}, {z})");
        }
    }
}

#[test]
fn heightmap_packs_entries_lowest_index_in_lowest_bits() {
    // 9 bits per entry, lowest entry in lowest bits of long 0.
    let mut h = ColumnHeightmap::new(384, 0);
    // Index 0 = (x=0, z=0); index 1 = (x=1, z=0); index 2 = (x=2, z=0).
    h.set(0, 0, 5);
    h.set(1, 0, 10);
    h.set(2, 0, 15);
    let expected = 5u64 | (10u64 << 9) | (15u64 << 18);
    assert_eq!(
        h.raw_longs()[0],
        expected,
        "entry n must occupy bits [n*bits, (n+1)*bits) of the long array"
    );
}

#[test]
fn heightmap_zero_init_returns_min_y_for_unprimed_columns() {
    let h = ColumnHeightmap::new(384, -64);
    assert_eq!(h.get(0, 0), -64);
    assert_eq!(h.get(BLOCKS::MASK, BLOCKS::MASK), -64);
}
