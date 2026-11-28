use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_time::{TimePlugin, TimeUpdateStrategy};
use mcrs_minecraft_engine::swing::{PlayerSwing, PlayerSwingPlugin};
use mcrs_minecraft_engine_item::{ItemCommandsExt, Selected, StackCommandsExt};
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_item_stack::{
    AttackAnimation, Count, InteractAnimation, ItemStack, SwingAnimation, SwingKind,
};
use mcrs_minecraft_player_holders::HandAction;
use mcrs_minecraft_vanilla_inventory::PlayerInventoryCommandsExt;

use crate::fixture::{STONE, gameplay_app, put};

fn setup_player(app: &mut App, mode: GameMode) -> Entity {
    if !app.is_plugin_added::<TimePlugin>() {
        app.add_plugins(TimePlugin)
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
        app.update();
    }
    if !app.is_plugin_added::<PlayerSwingPlugin>() {
        app.add_plugins(PlayerSwingPlugin);
    }
    let world = app.world_mut();
    world.insert_resource(BirthClock(10));
    let player = world
        .spawn((
            Player,
            Owner(PlayerSession(1)),
            PlayerGameMode(mode),
            PlayerSwing::default(),
            Selected(0),
        ))
        .id();
    world.flush();
    player
}

#[test]
fn hand_drop_triggers_default_main_whack_6_cooldown_and_ignores_empty_and_spectator() {
    let mut app = gameplay_app();
    let player = setup_player(&mut app, GameMode::Survival);

    app.world_mut()
        .commands()
        .entity(player)
        .hand_action(HandAction::DropOne);
    app.update();
    assert_eq!(
        app.world().get::<PlayerSwing>(player),
        Some(&PlayerSwing::default()),
        "SWG-SRC-003: dropping with an empty hand must not start a swing"
    );

    app.world_mut().resource_mut::<BirthClock>().0 = 20;
    let stack = ItemStack::new(STONE, Count::new(3).unwrap())
        .set(AttackAnimation(SwingAnimation {
            kind: SwingKind::Stab,
            duration: 20,
        }))
        .set(InteractAnimation(SwingAnimation {
            kind: SwingKind::None,
            duration: 20,
        }));
    app.world_mut()
        .commands()
        .spawn_stack(stack)
        .put_at(player, 0);
    app.world_mut().flush();

    app.world_mut()
        .commands()
        .entity(player)
        .hand_action(HandAction::DropOne);
    app.update();

    let tracker = app.world().get::<PlayerSwing>(player).unwrap().0;
    assert!(
        !tracker.can_start(23),
        "SWG-CMP-002: drop swing (whack, duration 6) must block at T + 3"
    );
    assert!(
        tracker.can_start(24),
        "SWG-CMP-002: drop swing (whack, duration 6) must unlock at T + 4 even with duration-20 components"
    );

    app.world_mut().resource_mut::<BirthClock>().0 = 30;
    app.world_mut()
        .commands()
        .entity(player)
        .hand_action(HandAction::DropStack);
    app.update();

    let tracker = app.world().get::<PlayerSwing>(player).unwrap().0;
    assert!(
        !tracker.can_start(33),
        "SWG-SRC-003: DropStack (DropAllItems) starts default main whack-6 cooldown"
    );
    assert!(
        tracker.can_start(34),
        "SWG-SRC-003: DropStack whack-6 unlocks at T + 4"
    );

    let spectator = setup_player(&mut app, GameMode::Spectator);
    put(app.world_mut(), spectator, 0, STONE, 1);
    app.world_mut().resource_mut::<BirthClock>().0 = 40;
    app.world_mut()
        .commands()
        .entity(spectator)
        .hand_action(HandAction::DropOne);
    app.update();
    assert_eq!(
        app.world().get::<PlayerSwing>(spectator),
        Some(&PlayerSwing::default()),
        "SWG-SRC-003: spectator drop must not trigger a swing"
    );
}
