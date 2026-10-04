use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::*;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_item::Items;
use mcrs_minecraft_level::experience::{
    AwardExperience, BlockDestroyed, DimensionRandom, ExperiencePlugin,
};
use mcrs_minecraft_protocol::item::Enchantments;
use mcrs_minecraft_world::item::{test_enchantment_registry, test_enchantments};

use crate::inventory_sync::value;

fn harness() -> App {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default());
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    app.insert_resource(test_enchantment_registry());
    app.insert_resource(test_enchantments());
    crate::support::insert_corpus(&mut app);
    app.add_plugins(ExperiencePlugin);
    app.add_systems(Update, collect_awards);
    app.init_resource::<Awarded>();
    app
}

#[derive(Resource, Default)]
struct Awarded(Vec<i32>);

fn collect_awards(mut reader: MessageReader<AwardExperience>, mut awarded: ResMut<Awarded>) {
    for event in reader.read() {
        awarded.0.push(event.amount);
    }
}

/// A diamond pickaxe stack whose only patch is the enchantment, the shape a
/// held tool actually has.
fn enchanted_pickaxe(app: &mut App, enchantment: &str, level: i32) -> Entity {
    let items = app.world().resource::<Items>().clone();
    let mut pickaxe = value("diamond_pickaxe", 1);
    pickaxe.components.set(Enchantments(vec![(
        ResourceKey::from_location(ResourceLocation::parse(enchantment).unwrap()),
        level,
    )]));
    mcrs_minecraft_inventory::value::spawn_stack(app.world_mut(), &pickaxe, &items).unwrap()
}

fn break_coal_ore(app: &mut App, tool: Option<Entity>, breaks: usize) -> Vec<i32> {
    let state = app
        .world()
        .resource::<Blocks>()
        .default_state("minecraft:coal_ore");
    // A message written last update is still readable this one; drain it before
    // counting so a run only sees its own breaks.
    app.update();
    app.world_mut().resource_mut::<Awarded>().0.clear();
    for _ in 0..breaks {
        app.world_mut().write_message(BlockDestroyed {
            state,
            pos: BlockPos::new(0, 64, 0),
            dim: Entity::PLACEHOLDER,
            tool,
            drop_experience: true,
        });
        app.update();
    }
    app.world().resource::<Awarded>().0.clone()
}

/// `minecraft:coal_ore` carries `UniformInt(0, 2)`, so a break pays 0, 1 or 2,
/// and only the non-zero ones reach `AwardExperience`. Silk touch states
/// `block_experience: set 0` and an unrelated enchantment states nothing, so
/// nothing here knows an enchantment by name.
#[test]
fn block_experience_is_the_block_sample_after_the_tool_effects() {
    let mut app = harness();

    let awarded = break_coal_ore(&mut app, None, 200);
    assert!(!awarded.is_empty(), "some break must pay out");
    for amount in &awarded {
        assert!(
            (1..=2).contains(amount),
            "coal ore paid {amount}, outside 0..=2"
        );
    }
    assert!(
        awarded.contains(&2),
        "the upper end of the range must be reachable: {awarded:?}"
    );

    let silk_touch = enchanted_pickaxe(&mut app, "minecraft:silk_touch", 1);
    let awarded = break_coal_ore(&mut app, Some(silk_touch), 200);
    assert!(
        awarded.is_empty(),
        "silk touch must suppress every payout, got {awarded:?}"
    );

    let efficiency = enchanted_pickaxe(&mut app, "minecraft:efficiency", 3);
    let awarded = break_coal_ore(&mut app, Some(efficiency), 200);
    assert!(!awarded.is_empty(), "efficiency must not suppress payouts");

    let stone = app
        .world()
        .resource::<Blocks>()
        .default_state("minecraft:stone");
    app.update();
    app.world_mut().resource_mut::<Awarded>().0.clear();
    for _ in 0..50 {
        app.world_mut().write_message(BlockDestroyed {
            state: stone,
            pos: BlockPos::new(0, 64, 0),
            dim: Entity::PLACEHOLDER,
            tool: None,
            drop_experience: true,
        });
        app.update();
    }
    assert!(
        app.world().resource::<Awarded>().0.is_empty(),
        "a block with no experience drop paid out"
    );

    *app.world_mut().resource_mut::<DimensionRandom>() = DimensionRandom::default();
    let first = break_coal_ore(&mut app, None, 50);
    *app.world_mut().resource_mut::<DimensionRandom>() = DimensionRandom::default();
    let second = break_coal_ore(&mut app, None, 50);
    assert_eq!(
        first, second,
        "the same dimension seed replays the same samples"
    );
}
