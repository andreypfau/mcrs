mod support;

use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::{AssetPlugin, AssetServer};
use bevy_ecs::prelude::*;
use mcrs_core::StaticRegistry;
use mcrs_voxel_math::BlockPos;
use mcrs_minecraft::world::experience::{
    AwardExperience, BlockDestroyed, DimensionRandom, ExperiencePlugin,
};
use mcrs_vanilla::item::component::Enchantments;
use mcrs_vanilla::block::definition::Blocks;
use mcrs_vanilla::enchantment::{EnchantmentData, register_all_enchantments};

fn harness() -> App {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default());
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    let asset_server = app.world().resource::<AssetServer>().clone();
    let mut enchantments = StaticRegistry::<EnchantmentData>::new();
    register_all_enchantments(&mut enchantments, &asset_server);
    enchantments.freeze();
    app.insert_resource(enchantments);
    let corpus = support::corpus(&app);
    app.insert_resource(corpus);
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

fn silk_touch_id(app: &App) -> u16 {
    app.world()
        .resource::<StaticRegistry<EnchantmentData>>()
        .id_of("minecraft:silk_touch")
        .expect("silk touch is registered")
        .raw() as u16
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

/// `minecraft:coal_ore` carries `UniformInt(0, 2)`, so a break pays 0, 1 or 2 —
/// and only the non-zero ones reach `AwardExperience`.
#[test]
fn coal_ore_experience_stays_within_its_declared_range() {
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
}

/// Silk touch states `block_experience: set 0`. Nothing here knows the
/// enchantment by name: the amount is zero because the effect says so.
#[test]
fn silk_touch_suppresses_block_experience() {
    let mut app = harness();
    let id = silk_touch_id(&app);
    let mut enchantments = Enchantments::default();
    enchantments.insert(id, 1);
    let tool = app.world_mut().spawn(enchantments).id();

    let awarded = break_coal_ore(&mut app, Some(tool), 200);
    assert!(
        awarded.is_empty(),
        "silk touch must suppress every payout, got {awarded:?}"
    );
}

/// An unrelated enchantment states no `block_experience`, so the amount is the
/// block's own sample.
#[test]
fn an_unrelated_enchantment_leaves_block_experience_alone() {
    let mut app = harness();
    let efficiency = app
        .world()
        .resource::<StaticRegistry<EnchantmentData>>()
        .id_of("minecraft:efficiency")
        .expect("efficiency is registered")
        .raw() as u16;
    let mut enchantments = Enchantments::default();
    enchantments.insert(efficiency, 3);
    let tool = app.world_mut().spawn(enchantments).id();

    let awarded = break_coal_ore(&mut app, Some(tool), 200);
    assert!(!awarded.is_empty(), "efficiency must not suppress payouts");
}

/// A block with no `mcrs:experience_drop` pays nothing at all.
#[test]
fn a_block_without_an_experience_drop_pays_nothing() {
    let mut app = harness();
    let state = app
        .world()
        .resource::<Blocks>()
        .default_state("minecraft:stone");
    for _ in 0..50 {
        app.world_mut().write_message(BlockDestroyed {
            state,
            pos: BlockPos::new(0, 64, 0),
            dim: Entity::PLACEHOLDER,
            tool: None,
            drop_experience: true,
        });
        app.update();
    }
    assert!(app.world().resource::<Awarded>().0.is_empty());
}

/// The dimension owns the stream: the same seed replays the same samples.
#[test]
fn the_sample_comes_from_the_dimension_random() {
    let mut app = harness();
    let first = break_coal_ore(&mut app, None, 50);
    *app.world_mut().resource_mut::<DimensionRandom>() = DimensionRandom::default();
    let second = break_coal_ore(&mut app, None, 50);
    assert_eq!(first, second);
}
