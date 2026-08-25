use std::sync::OnceLock;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use bevy_math::Vec3;
use mcrs_core::voxel_shape::Aabb;
use mcrs_protocol::BlockStateId;
use mcrs_vanilla::block::definition::schema::{Instrument, PropertyValue, RenderShape};
use mcrs_vanilla::block::definition::{
    BlockDefinitions, BlockStateData, BlockStateFlags, LoadReport, load_block_definitions,
};
use mcrs_vanilla::material::PushReaction;
use mcrs_vanilla::material::map::MapColor;

fn corpus() -> &'static (BlockDefinitions, LoadReport) {
    static CORPUS: OnceLock<(BlockDefinitions, LoadReport)> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        });
        let asset_server = app.world().resource::<AssetServer>().clone();
        load_block_definitions(&asset_server).expect("the corpus loads")
    })
}

fn string(value: &str) -> PropertyValue {
    PropertyValue::Str(value.into())
}

fn aabb(min: [f32; 3], max: [f32; 3]) -> Aabb {
    Aabb {
        min: Vec3::from(min),
        max: Vec3::from(max),
    }
}

#[test]
fn every_file_parses_and_the_states_tile_the_id_space() {
    let (definitions, report) = corpus();
    println!(
        "{} files, {} states, {} permutations, {} interned shapes, \
         {} bytes ({} per state), loaded in {:?}",
        report.files,
        report.states,
        report.permutations,
        report.shapes,
        report.table_bytes,
        size_of::<BlockStateData>(),
        report.elapsed
    );
    assert_eq!(report.files, 1286);
    assert_eq!(report.states, 35723);
    assert_eq!(definitions.state_count(), 35723);
    assert_eq!(definitions.blocks().len(), 1286);

    let mut claimed = vec![false; report.states];
    for block in definitions.blocks() {
        for offset in 0..block.state_count {
            let state = block.base_state_id.0 as usize + offset as usize;
            assert!(!claimed[state], "state {state} is claimed twice");
            claimed[state] = true;
        }
    }
    assert!(claimed.iter().all(|&c| c), "a state is claimed by no block");
}

#[test]
fn every_default_state_lies_in_its_block_and_named_defaults_reconstruct() {
    let (definitions, _) = corpus();
    for block in definitions.blocks() {
        let offset = block.default_state_id.0 - block.base_state_id.0;
        assert!(
            offset < block.state_count,
            "{} default state {} is outside its range",
            block.identifier.as_str(),
            block.default_state_id.0
        );
    }

    // The corpus states no default property values, so the reconstruction is
    // checked against blocks whose vanilla default is known. A reversed
    // varies-fastest end reproduces neither.
    let stairs = definitions.block("minecraft:oak_stairs").unwrap();
    assert_eq!(
        stairs.state_id(&[
            ("facing", string("north")),
            ("half", string("bottom")),
            ("shape", string("straight")),
            ("waterlogged", PropertyValue::Bool(false)),
        ]),
        Some(stairs.default_state_id)
    );

    let note_block = definitions.block("minecraft:note_block").unwrap();
    assert_eq!(
        note_block.state_id(&[
            ("instrument", string("harp")),
            ("note", PropertyValue::Int(0)),
            ("powered", PropertyValue::Bool(false)),
        ]),
        Some(note_block.default_state_id)
    );

    let water = definitions.block("minecraft:water").unwrap();
    assert_eq!(
        water.state_id(&[("level", PropertyValue::Int(0))]),
        Some(water.default_state_id)
    );
}

#[test]
fn stone_has_one_state_and_a_full_cube() {
    let (definitions, _) = corpus();
    let stone = definitions.block("minecraft:stone").unwrap();
    assert_eq!(stone.state_count, 1);
    assert_eq!(stone.base_state_id, BlockStateId(1));
    assert_eq!(stone.base_state_id, stone.default_state_id);
    assert!(stone.properties.0.is_empty());

    let state = definitions.state(stone.default_state_id);
    assert_eq!(state.hardness, 1.5);
    assert_eq!(state.explosion_resistance, 6.0);
    assert_eq!(state.light_dampening, 15);
    assert_eq!(state.light_emission, 0);
    assert_eq!(state.friction, 0.6);
    assert_eq!(
        state.map_color,
        MapColor {
            r: 0x70,
            g: 0x70,
            b: 0x70
        }
    );
    assert_eq!(state.push_reaction, PushReaction::Normal);
    assert_eq!(state.instrument, Instrument::Basedrum);
    assert_eq!(state.render_shape, RenderShape::Model);
    assert_eq!(state.fluid, None);
    assert!(
        state
            .flags
            .contains(BlockStateFlags::REQUIRES_CORRECT_TOOL_FOR_DROPS)
    );
    assert!(state.flags.contains(BlockStateFlags::REDSTONE_CONDUCTOR));
    assert!(!state.flags.contains(BlockStateFlags::IS_AIR));
    assert_eq!(
        definitions.shape(state.collision_shape),
        [aabb([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])]
    );
}

#[test]
fn water_carries_its_fluid_state_per_level() {
    let (definitions, _) = corpus();
    let water = definitions.block("minecraft:water").unwrap();
    assert_eq!(water.state_count, 16);

    let source = definitions.state(water.default_state_id).fluid.unwrap();
    assert_eq!(definitions.fluid(source.fluid).as_str(), "minecraft:water");
    assert_eq!(source.level, 8);
    assert!(source.source);

    let falling = definitions
        .state(water.state_id(&[("level", PropertyValue::Int(1))]).unwrap())
        .fluid
        .unwrap();
    assert_eq!(
        definitions.fluid(falling.fluid).as_str(),
        "minecraft:flowing_water"
    );
    assert_eq!(falling.level, 7);
    assert!(!falling.source);

    assert!(
        definitions
            .shape(definitions.state(water.default_state_id).collision_shape)
            .is_empty()
    );
}

#[test]
fn oak_stairs_collision_follows_facing_and_half_but_not_waterlogging() {
    let (definitions, _) = corpus();
    let stairs = definitions.block("minecraft:oak_stairs").unwrap();
    assert_eq!(stairs.state_count, 80);

    let shape_of = |facing: &str, half: &str, waterlogged: bool| {
        let id = stairs
            .state_id(&[
                ("facing", string(facing)),
                ("half", string(half)),
                ("shape", string("straight")),
                ("waterlogged", PropertyValue::Bool(waterlogged)),
            ])
            .unwrap();
        definitions.state(id).collision_shape
    };

    assert_eq!(
        definitions.shape(shape_of("north", "bottom", false)),
        [
            aabb([0.0, 0.0, 0.0], [1.0, 0.5, 1.0]),
            aabb([0.0, 0.5, 0.0], [1.0, 1.0, 0.5]),
        ]
    );
    assert_eq!(
        shape_of("north", "bottom", true),
        shape_of("north", "bottom", false)
    );
    assert_ne!(
        shape_of("south", "bottom", false),
        shape_of("north", "bottom", false)
    );
    assert_ne!(
        shape_of("north", "top", false),
        shape_of("north", "bottom", false)
    );

    let dry = stairs
        .state_id(&[
            ("facing", string("north")),
            ("half", string("bottom")),
            ("shape", string("straight")),
            ("waterlogged", PropertyValue::Bool(false)),
        ])
        .unwrap();
    let wet = stairs
        .state_id(&[
            ("facing", string("north")),
            ("half", string("bottom")),
            ("shape", string("straight")),
            ("waterlogged", PropertyValue::Bool(true)),
        ])
        .unwrap();
    assert_eq!(definitions.state(dry).fluid, None);
    assert_eq!(definitions.state(wet).fluid.unwrap().level, 8);
    assert_eq!(definitions.state(dry).light_dampening, 0);
    assert_eq!(definitions.state(wet).light_dampening, 1);
}

#[test]
fn note_block_carries_a_large_integer_property() {
    let (definitions, _) = corpus();
    let note_block = definitions.block("minecraft:note_block").unwrap();
    assert_eq!(note_block.state_count, 27 * 25 * 2);
    let note = &note_block.properties.0[1];
    assert_eq!(&*note.name, "note");
    assert_eq!(note.values.first(), Some(&PropertyValue::Int(0)));
    assert_eq!(note.values.last(), Some(&PropertyValue::Int(24)));

    let last = note_block
        .state_id(&[
            ("instrument", string("custom_head")),
            ("note", PropertyValue::Int(24)),
            ("powered", PropertyValue::Bool(false)),
        ])
        .unwrap();
    assert_eq!(
        last.0,
        note_block.base_state_id.0 + note_block.state_count - 1
    );
    assert_eq!(definitions.state(last).hardness, 0.8);
}

#[test]
fn torch_is_not_a_cube() {
    let (definitions, _) = corpus();
    let torch = definitions.block("minecraft:torch").unwrap();
    let state = definitions.state(torch.default_state_id);
    assert_eq!(state.light_emission, 14);
    assert!(definitions.shape(state.collision_shape).is_empty());
    assert_eq!(
        definitions.shape(state.selection_shape),
        [aabb([0.375, 0.0, 0.375], [0.625, 0.625, 0.625])]
    );
    assert!(!state.flags.contains(BlockStateFlags::IS_SOLID_RENDER));
    assert!(
        state
            .flags
            .contains(BlockStateFlags::PROPAGATES_SKYLIGHT_DOWN)
    );
}

#[test]
fn the_dense_escape_hatch_varies_a_button_selection_box_per_state() {
    let (definitions, _) = corpus();
    let button = definitions.block("minecraft:acacia_button").unwrap();
    let shape_of = |face: &str, facing: &str, powered: bool| {
        let id = button
            .state_id(&[
                ("face", string(face)),
                ("facing", string(facing)),
                ("powered", PropertyValue::Bool(powered)),
            ])
            .unwrap();
        definitions.state(id).selection_shape
    };
    assert_ne!(
        shape_of("floor", "north", true),
        shape_of("floor", "north", false)
    );
    assert_ne!(
        shape_of("wall", "north", false),
        shape_of("floor", "north", false)
    );
}
