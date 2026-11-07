#![expect(
    clippy::unwrap_used,
    reason = "the corpus loaders fail the test by panicking"
)]

use std::time::Instant;

use mcrs_minecraft_collision_table::collision_table;
use mcrs_minecraft_collision_table::widen;
use mcrs_minecraft_physics::Axis;
use mcrs_minecraft_physics::BlockFlags;
use mcrs_minecraft_physics::InsideShape;
use mcrs_minecraft_physics::ShapeClass;
use mcrs_voxel::BlockStateId;

mod common;

use common::definitions;
use common::table;
use common::tags;

fn state(name: &str) -> BlockStateId {
    definitions().default_state(name).unwrap()
}

#[test]
fn every_corpus_coordinate_widens_exactly() {
    let definitions = definitions();
    let mut checked = 0;
    for id in definitions.states() {
        for bounds in definitions.shape(definitions.state(id).collision_shape) {
            for value in bounds
                .min
                .to_array()
                .into_iter()
                .chain(bounds.max.to_array())
            {
                // Each corpus value is a multiple of half a pixel, which f32 and f64 both hold.
                assert_eq!(
                    (value * 32.0).fract(),
                    0.0,
                    "{value} is off the half-pixel grid"
                );
                assert_eq!(widen(value), f64::from(value));
                checked += 1;
            }
        }
    }
    assert!(checked > 90_000, "only {checked} coordinates");
    println!("{checked} corpus coordinates widen exactly");
}

#[test]
fn a_rounded_corpus_value_widens_to_its_decimal() {
    // 8.0001 pixels, as the corpus rounds an f32 constant, is 80001 / 160000 blocks.
    let value = (8.0001_f64 / 16.0) as f32;
    assert_eq!(widen(value), 80001.0 / 160000.0);
}

#[test]
fn every_corpus_state_round_trips() {
    let definitions = definitions();
    let table = table();
    assert_eq!(table.len(), definitions.state_count());
    for id in definitions.states() {
        let boxes: Vec<[f64; 6]> = definitions
            .shape(definitions.state(id).collision_shape)
            .iter()
            .map(|bounds| {
                let [a, b, c] = bounds.min.to_array().map(widen);
                let [d, e, f] = bounds.max.to_array().map(widen);
                [a, b, c, d, e, f]
            })
            .collect();
        let class = table.state(id).class;
        let shape = match class {
            ShapeClass::Scaffolding { .. } | ShapeClass::PowderSnow => continue,
            ShapeClass::Empty => {
                assert!(boxes.is_empty(), "state {id:?} lost its boxes");
                continue;
            }
            ShapeClass::FullCube => table.shape(table.full_cube()).unwrap(),
            ShapeClass::OneBox(shape) | ShapeClass::Compound(shape) => table.shape(shape).unwrap(),
        };
        assert_eq!(
            matches!(class, ShapeClass::OneBox(_) | ShapeClass::FullCube),
            boxes.len() == 1
        );
        for bounds in &boxes {
            for axis in Axis::ALL {
                for face in [bounds[axis.index()], bounds[axis.index() + 3]] {
                    let lines = shape.lines(axis);
                    assert!(
                        lines.iter().any(|line| (line - face).abs() < 1.0e-7),
                        "state {id:?}: face {face} is not a line of {lines:?}"
                    );
                }
            }
        }
        let [xs, ys, zs] = Axis::ALL.map(|axis| shape.lines(axis));
        for (x, pair_x) in xs.windows(2).enumerate() {
            for (y, pair_y) in ys.windows(2).enumerate() {
                for (z, pair_z) in zs.windows(2).enumerate() {
                    let middle = [
                        (pair_x[0] + pair_x[1]) / 2.0,
                        (pair_y[0] + pair_y[1]) / 2.0,
                        (pair_z[0] + pair_z[1]) / 2.0,
                    ];
                    let inside = boxes.iter().any(|bounds| contains(bounds, middle));
                    assert_eq!(
                        shape.is_solid(x, y, z),
                        inside,
                        "state {id:?}, cell ({x}, {y}, {z})"
                    );
                }
            }
        }
    }
}

#[test]
fn a_carpet_gives_its_faces_as_lines() {
    let table = table();
    let ShapeClass::OneBox(shape) = table.state(state("minecraft:white_carpet")).class else {
        panic!("a carpet is one box");
    };
    assert_eq!(
        table.shape(shape).unwrap().lines(Axis::Y),
        [0.0, 1.0 / 16.0]
    );
}

#[test]
fn friction_is_the_corpus_value_with_the_default_of_0_6f() {
    let table = table();
    assert_eq!(
        table.state(state("minecraft:stone")).motion.friction,
        0.6_f32
    );
    assert_eq!(table.state(BlockStateId::AIR).motion.friction, 0.6_f32);
    assert_eq!(
        table.state(state("minecraft:ice")).motion.friction,
        0.98_f32
    );
    assert_eq!(
        table.state(state("minecraft:blue_ice")).motion.friction,
        0.989_f32
    );
    assert_eq!(
        table.state(state("minecraft:slime_block")).motion.friction,
        0.8_f32
    );
}

#[test]
fn the_tags_and_names_set_their_flags_and_motion() {
    let table = table();
    let fence = table.state(state("minecraft:oak_fence"));
    assert!(fence.flags.contains(BlockFlags::FENCE | BlockFlags::LARGE));
    assert!(
        table
            .state(state("minecraft:cobblestone_wall"))
            .flags
            .contains(BlockFlags::WALL)
    );
    assert!(
        table
            .state(state("minecraft:oak_fence_gate"))
            .flags
            .contains(BlockFlags::FENCE_GATE)
    );
    assert!(
        table
            .state(state("minecraft:ladder"))
            .flags
            .contains(BlockFlags::CLIMBABLE)
    );
    let honey = table.state(state("minecraft:honey_block"));
    assert!(honey.flags.contains(BlockFlags::SUPPRESSES_BOUNCE));
    assert_eq!(
        (honey.motion.speed_factor, honey.motion.jump_factor),
        (0.4, 0.5)
    );
    assert_eq!(table.state(state("minecraft:red_bed")).motion.bounce, 0.75);
    assert_eq!(
        table.state(state("minecraft:slime_block")).motion.bounce,
        1.0
    );
    assert_eq!(
        table
            .state(state("minecraft:soul_sand"))
            .motion
            .speed_factor,
        0.4
    );
    let water = table.state(state("minecraft:water"));
    assert!(
        water
            .flags
            .contains(BlockFlags::FLUID | BlockFlags::OWN_SPEED_FACTOR)
    );
    assert!(
        table
            .state(state("minecraft:moving_piston"))
            .flags
            .contains(BlockFlags::MOVING_PISTON)
    );
    assert_eq!(
        table.state(state("minecraft:powder_snow")).class,
        ShapeClass::PowderSnow
    );
}

#[test]
fn rch_007_marks_exactly_the_blocks_with_an_inside_action() {
    let table = table();
    let marked = |name: &str| {
        table
            .state(state(name))
            .flags
            .contains(BlockFlags::INSIDE_ACTION)
    };
    for name in [
        "minecraft:wheat",
        "minecraft:pitcher_crop",
        "minecraft:water_cauldron",
        "minecraft:lava_cauldron",
        "minecraft:soul_fire",
        "minecraft:soul_campfire",
        "minecraft:cactus",
        "minecraft:open_eyeblossom",
        "minecraft:bubble_column",
        "minecraft:oak_pressure_plate",
        "minecraft:ladder",
    ] {
        assert!(marked(name), "{name} has an inside action");
    }
    for name in [
        "minecraft:pumpkin_stem",
        "minecraft:melon_stem",
        "minecraft:cauldron",
        "minecraft:stone",
    ] {
        assert!(!marked(name), "{name} has no inside action");
    }
}

#[test]
fn inp_021_marks_the_states_that_cause_suffocation() {
    let definitions = definitions();
    let table = table();
    let suffocates = |id: BlockStateId| table.state(id).flags.contains(BlockFlags::SUFFOCATES);
    for name in [
        "minecraft:stone",
        "minecraft:farmland",
        "minecraft:dirt_path",
        "minecraft:soul_sand",
        "minecraft:piston",
    ] {
        assert!(suffocates(state(name)), "{name} causes suffocation");
    }
    for name in [
        "minecraft:shulker_box",
        "minecraft:oak_leaves",
        "minecraft:glass",
        "minecraft:white_stained_glass",
        "minecraft:waxed_oxidized_copper_grate",
        "minecraft:mangrove_roots",
        "minecraft:stone_slab",
        "minecraft:air",
    ] {
        assert!(
            !suffocates(state(name)),
            "{name} does not cause suffocation"
        );
    }
    let piston = definitions.block("minecraft:piston").unwrap();
    for offset in 0..piston.state_count {
        let id = piston.state_at(offset).unwrap();
        let extended = piston.value_of(id, "extended").unwrap().to_string() == "true";
        assert_eq!(suffocates(id), !extended);
    }
}

#[test]
fn rch_008_gives_the_inside_shapes_of_the_table() {
    let table = table();
    let shape = |name: &str| table.state(state(name)).inside_shape;
    assert_eq!(shape("minecraft:stone"), InsideShape::FullCube);
    assert_eq!(shape("minecraft:powder_snow"), InsideShape::Collision);
    let InsideShape::Boxes(range) = shape("minecraft:lava_cauldron") else {
        panic!("a lava cauldron has boxes");
    };
    let top = table
        .inside_boxes(range)
        .iter()
        .map(|bounds| bounds.max.y)
        .fold(f64::NEG_INFINITY, f64::max);
    assert_eq!(top, 1.0, "the cauldron walls reach the top");
    let InsideShape::Boxes(range) = shape("minecraft:end_portal") else {
        panic!("an end portal has a box");
    };
    let [portal] = table.inside_boxes(range) else {
        panic!("an end portal has one box");
    };
    assert_eq!((portal.min.y, portal.max.y), (6.0 / 16.0, 12.0 / 16.0));
}

#[test]
fn scaffolding_gives_the_bottom_slab_only_with_distance_and_bottom() {
    let definitions = definitions();
    let table = table();
    let block = definitions.block("minecraft:scaffolding").unwrap();
    for offset in 0..block.state_count {
        let id = block.state_at(offset).unwrap();
        let distance = block.value_of(id, "distance").unwrap().to_string();
        let bottom = block.value_of(id, "bottom").unwrap().to_string();
        let expected = distance != "0" && bottom == "true";
        assert_eq!(
            table.state(id).class,
            ShapeClass::Scaffolding { bottom: expected }
        );
    }
}

#[test]
fn the_table_builds_in_less_than_100_ms() {
    let (definitions, tags) = (definitions(), tags());
    let started = Instant::now();
    let built = collision_table(definitions, tags);
    let elapsed = started.elapsed();
    println!("built {} states in {elapsed:?}", built.len());
    if !cfg!(debug_assertions) {
        assert!(elapsed.as_millis() < 100, "the build took {elapsed:?}");
    }
}

fn contains(bounds: &[f64; 6], point: [f64; 3]) -> bool {
    let [x0, y0, z0, x1, y1, z1] = *bounds;
    let [x, y, z] = point;
    x0 < x && x < x1 && y0 < y && y < y1 && z0 < z && z < z1
}
