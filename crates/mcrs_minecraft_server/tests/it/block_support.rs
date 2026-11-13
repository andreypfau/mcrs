//! Block support in a dimension world under the vanilla block rules: a set that removes the base
//! of a plant removes the plant through the shape updates of the set, and the plant drops the
//! items of its block table.

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_math::IVec3;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_engine::column_streams::ColumnStreams;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_registries::{install_item_table, test_item_table};
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::palette::air_section;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::storage::random_ticks::RandomTickIndex;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{Count, Kind};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_server::{
    VanillaBlocksPlugin, VanillaEntityPhysicsPlugin, VanillaItemEntityPlugin,
};
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos, SectionPos};

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
        let mut app = host_app::make_host_app();
        install_item_table(app.world_mut(), test_item_table()).unwrap();
        app.add_plugins((
            VanillaBlocksPlugin,
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

    /// Sets each block with flags 3, from the bottom up.
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

/// SUP-RUN-001, SUP-VEG-001, R6: a set that removes the base of a plant destroys the plant, and
/// the plant drops its item.
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

/// SUP-RUN-001: a set with keep-shape makes no shape update, so the plant stays without its
/// base until a shape update comes from below.
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

/// SUP-VEG-002, R5, R6: the base of a sunflower goes, so the lower part goes, and its removal
/// removes the upper part in the same tick. One sunflower drops.
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

/// SUP-VEG-002: a change beside a part of a tall plant, not above or below it, leaves the plant.
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
