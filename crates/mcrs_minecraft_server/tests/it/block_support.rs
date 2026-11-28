use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_math::IVec3;
use std::sync::Arc;

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::{Blocks, build_block_definitions};
use mcrs_minecraft_engine::column_streams::ColumnStreams;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_registries::{
    install_item_table, install_registries, test_corpus, test_item_table,
};
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::palette::air_section;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::storage::random_ticks::RandomTickIndex;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{Count, Kind};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_server::{
    VanillaBlocksPlugin, VanillaEntityPhysicsPlugin, VanillaItemEntityPlugin,
};
use mcrs_minecraft_simulation::{BlockUpdateFlags, ChainLimit, KernelConfig};
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos, SectionPos};
use serde_json::{Value, json};

use crate::host_app::{self, PlayServer};

const P: BlockPos = BlockPos::new(10, 64, 8);
const BELOW: BlockPos = BlockPos::new(10, 63, 8);
const ABOVE: BlockPos = BlockPos::new(10, 65, 8);
const KEEP_SHAPE: BlockUpdateFlags = BlockUpdateFlags::CLIENTS.union(BlockUpdateFlags::KNOWN_SHAPE);

struct World {
    server: PlayServer,
    dimension: Entity,
}

impl World {
    fn new() -> Self {
        Self::start(None, KernelConfig::default().chain_limit)
    }

    fn start(blocks: Option<Blocks>, chain_limit: ChainLimit) -> Self {
        let mut app = host_app::make_host_app();
        if let Some(blocks) = blocks {
            let items = test_corpus().1.clone();
            install_registries(app.world_mut(), test_registries().clone(), blocks, items);
        }
        install_item_table(app.world_mut(), test_item_table()).unwrap();
        app.add_plugins((
            VanillaBlocksPlugin { chain_limit },
            VanillaEntityPhysicsPlugin,
            VanillaItemEntityPlugin,
        ));
        let mut server = PlayServer::start(app, &[], 0);
        let (anchor, _) = server.join(Uuid::from_u128(1), "Watcher");
        let player = server.player(anchor);
        let world = server.world();
        let dimension = world.get::<InDimension>(player).unwrap().0;
        world
            .entity_mut(dimension)
            .insert(ColumnStreams::subscribed_to([ColumnPos::from(P)]));
        let mut this = World { server, dimension };
        for y in [BELOW.y, ABOVE.y] {
            this.add_section(BlockPos::new(P.x, y, P.z));
        }
        this
    }

    fn add_section(&mut self, pos: BlockPos) {
        let section_pos = SectionPos::from(pos);
        let world = self.server.world();
        let index = world.get::<SectionIndex>(self.dimension).unwrap();
        if index.blocks(section_pos).is_some() {
            return;
        }
        let section = world.spawn((section_pos, InDimension(self.dimension))).id();
        let mut random_ticks = world.remove_resource::<RandomTickIndex>();
        let mut index = world.get_mut::<SectionIndex>(self.dimension).unwrap();
        index.insert(section_pos, section);
        index.land(
            section_pos,
            section,
            air_section(),
            random_ticks.as_mut(),
            false,
        );
        if let Some(random_ticks) = random_ticks {
            world.insert_resource(random_ticks);
        }
    }

    fn state(&mut self, block: &str, values: &[(&str, &str)]) -> BlockStateId {
        let blocks = self.server.world().resource::<Blocks>();
        let entry = blocks.block(block).unwrap();
        values
            .iter()
            .fold(entry.default_state_id, |state, (name, value)| {
                entry.with_text(state, name, value).unwrap()
            })
    }

    fn set(&mut self, pos: BlockPos, state: BlockStateId, flags: BlockUpdateFlags) {
        let dimension = self.dimension;
        self.server
            .world()
            .resource_mut::<Messages<BlockSetRequest>>()
            .write(BlockSetRequest {
                dimension,
                pos,
                new_state: state,
                flags,
                recursion_left: 512,
            });
        self.server.ticks_all(2);
    }

    fn build(&mut self, blocks: &[(BlockPos, BlockStateId)]) {
        for (pos, state) in blocks {
            self.set(*pos, *state, BlockUpdateFlags::ALL);
        }
    }

    fn block_at(&mut self, pos: BlockPos) -> BlockStateId {
        let index = self
            .server
            .world()
            .get::<SectionIndex>(self.dimension)
            .unwrap();
        index
            .blocks(SectionPos::from(pos))
            .unwrap()
            .get(LocalPos::from(pos))
    }

    fn item_entities(&mut self) -> Vec<(Kind, u8)> {
        self.server.ticks_all(1);
        let world = self.server.world();
        let mut items = world.query_filtered::<(&Kind, &Count), With<Dropped>>();
        items
            .iter(world)
            .map(|(&kind, count)| (kind, count.get()))
            .collect()
    }
}

fn one(item: Item) -> Vec<(Kind, u8)> {
    vec![(Kind(item.id().number()), 1)]
}

#[test]
fn sup_run_001_a_plant_whose_base_goes_is_destroyed_and_drops_its_item() {
    let mut world = World::new();
    let dirt = world.state("minecraft:dirt", &[]);
    let poppy = world.state("minecraft:poppy", &[]);
    world.build(&[(BELOW, dirt), (P, poppy)]);

    world.set(BELOW, BlockStateId::AIR, BlockUpdateFlags::ALL);

    assert_eq!(world.block_at(P), BlockStateId::AIR);
    assert_eq!(world.item_entities(), one(Item::Poppy));
}

#[test]
fn sim_set_004_a_waterlogged_plant_whose_base_goes_leaves_a_water_source() {
    let mut world = World::new();
    let dirt = world.state("minecraft:dirt", &[]);
    let propagule = world.state(
        "minecraft:mangrove_propagule",
        &[("age", "4"), ("hanging", "false"), ("waterlogged", "true")],
    );
    let source = world.state("minecraft:water", &[("level", "0")]);
    world.build(&[(BELOW, dirt), (P, propagule)]);
    assert_eq!(world.block_at(P), propagule);

    world.set(BELOW, BlockStateId::AIR, BlockUpdateFlags::ALL);

    assert_eq!(world.block_at(P), source);
    assert_eq!(world.item_entities(), one(Item::MangrovePropagule));
}

#[test]
fn sup_run_001_a_plant_stays_after_a_set_with_keep_shape() {
    let mut world = World::new();
    let dirt = world.state("minecraft:dirt", &[]);
    let stone = world.state("minecraft:stone", &[]);
    let poppy = world.state("minecraft:poppy", &[]);
    world.build(&[(BELOW, dirt), (P, poppy)]);

    world.set(BELOW, BlockStateId::AIR, KEEP_SHAPE);
    assert_eq!(world.block_at(P), poppy);

    world.set(BELOW, stone, BlockUpdateFlags::ALL);
    assert_eq!(world.block_at(P), BlockStateId::AIR);
}

#[test]
fn sup_veg_002_both_parts_of_a_sunflower_go_with_its_base() {
    let mut world = World::new();
    let dirt = world.state("minecraft:dirt", &[]);
    let lower = world.state("minecraft:sunflower", &[("half", "lower")]);
    let upper = world.state("minecraft:sunflower", &[("half", "upper")]);
    world.build(&[(BELOW, dirt), (P, lower), (ABOVE, upper)]);
    assert_eq!([world.block_at(P), world.block_at(ABOVE)], [lower, upper]);

    world.set(BELOW, BlockStateId::AIR, BlockUpdateFlags::ALL);

    assert_eq!(
        [world.block_at(P), world.block_at(ABOVE)],
        [BlockStateId::AIR; 2]
    );
    assert_eq!(world.item_entities(), one(Item::Sunflower));
}

fn poppy_testing_all_directions() -> Blocks {
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/mcrs/block_definition"
    );
    let files = std::fs::read_dir(directory)
        .expect("the corpus")
        .map(|entry| entry.expect("a corpus file").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .map(|path| {
            let mut file: Value =
                serde_json::from_slice(&std::fs::read(&path).expect("a corpus file"))
                    .expect("json");
            let block = &mut file["minecraft:block"];
            if block["description"]["identifier"] == "minecraft:poppy" {
                block["components"]["minecraft:placement_filter"] = json!({
                    "predicate": {"type": "minecraft:matching_blocks", "offset": [0, -1, 0],
                        "blocks": "minecraft:dirt"},
                    "tested_directions": "all",
                });
            }
            (
                path.display().to_string(),
                serde_json::to_vec(&file).expect("json"),
            )
        });
    let names: Vec<&str> = Block::ALL
        .iter()
        .map(|block| block.as_static_str())
        .collect();
    let (definitions, _) = build_block_definitions(&names, files).expect("the patched corpus");
    Blocks(Arc::new(definitions))
}

#[test]
fn sup_run_001_an_unavailable_base_keeps_the_plant() {
    let mut world = World::start(
        Some(poppy_testing_all_directions()),
        KernelConfig::default().chain_limit,
    );
    let stone = world.state("minecraft:stone", &[]);
    let poppy = world.state("minecraft:poppy", &[]);
    let lowest = BlockPos::new(P.x, BELOW.y - 15, P.z);
    world.set(lowest, poppy, KEEP_SHAPE);
    assert_eq!(world.block_at(lowest), poppy);

    world.set(lowest + IVec3::X, stone, BlockUpdateFlags::ALL);

    assert_eq!(world.block_at(lowest), poppy);
}

#[test]
fn sup_veg_002_a_side_neighbour_leaves_a_tall_plant() {
    let mut world = World::new();
    let dirt = world.state("minecraft:dirt", &[]);
    let stone = world.state("minecraft:stone", &[]);
    let lower = world.state("minecraft:sunflower", &[("half", "lower")]);
    let upper = world.state("minecraft:sunflower", &[("half", "upper")]);
    world.build(&[(BELOW, dirt), (P, lower), (ABOVE, upper)]);

    world.set(P + IVec3::X, stone, BlockUpdateFlags::ALL);
    world.set(ABOVE + IVec3::X, stone, BlockUpdateFlags::ALL);

    assert_eq!([world.block_at(P), world.block_at(ABOVE)], [lower, upper]);
}

const COLUMN: i32 = 8;

fn carpet_column(world: &mut World) -> Vec<BlockStateId> {
    (0..COLUMN)
        .map(|height| world.block_at(BlockPos::new(P.x, P.y + height, P.z)))
        .collect()
}

#[test]
fn sim_nbr_004_the_chain_limit_of_the_plugin_stops_a_support_cascade() {
    for (limit, gone) in [
        (ChainLimit::Limited(20), 3),
        (ChainLimit::Unlimited, COLUMN),
    ] {
        let mut world = World::start(None, limit);
        let dirt = world.state("minecraft:dirt", &[]);
        let carpet = world.state("minecraft:white_carpet", &[]);
        world.set(BELOW, dirt, BlockUpdateFlags::ALL);
        for height in 0..COLUMN {
            let pos = BlockPos::new(P.x, P.y + height, P.z);
            world.set(pos, carpet, BlockUpdateFlags::ALL);
        }
        assert_eq!(
            carpet_column(&mut world),
            vec![carpet; COLUMN as usize],
            "the column stands with {limit:?}"
        );

        world.set(BELOW, BlockStateId::AIR, BlockUpdateFlags::ALL);

        let expected: Vec<BlockStateId> = (0..COLUMN)
            .map(|height| {
                if height < gone {
                    BlockStateId::AIR
                } else {
                    carpet
                }
            })
            .collect();
        assert_eq!(
            carpet_column(&mut world),
            expected,
            "with {limit:?}, the first {gone} carpets lose their support: each pop offers one \
             neighbours-around request and six shape requests, the carpet above goes with the \
             sixth, so the requests 1, 8 and 15 pop three carpets and a limit of 20 discards \
             the twenty-second"
        );
        assert_eq!(
            world.item_entities().len(),
            gone as usize,
            "with {limit:?}, each popped carpet drops once"
        );
    }
}
