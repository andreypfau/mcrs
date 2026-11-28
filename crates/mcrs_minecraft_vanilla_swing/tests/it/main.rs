#![expect(clippy::unwrap_used, reason = "a test reports a failure by panicking")]

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use bevy_time::{TimePlugin, TimeUpdateStrategy};
use bytes::Bytes;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine::loaded_client::{LoadedClientPlugin, UnloadedClient};
use mcrs_minecraft_engine::player_state::PlayerFileState;
use mcrs_minecraft_engine::swing::{PlayerSwing, PlayerSwingPlugin};
use mcrs_minecraft_engine_item::{ItemCommandsExt, ItemPlugin, Selected, Slots, StackCommandsExt};
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_item_stack::{
    AttackAnimation, Count, ItemStack, Kind, SwingAnimation, SwingKind,
};
use mcrs_minecraft_item_table::{DefinitionFile, ItemTable, bake};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::{Encode, VarInt};
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::{ServerboundPlayerAction, ServerboundPunch};
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_vanilla_swing::SwingPlugin;
use mcrs_voxel::BlockPos;

const STICK: Kind = Kind(1);
const SPEAR: Kind = Kind(2);

fn test_app() -> App {
    let mut app = App::new();
    let files = [
        r#"{"minecraft:item":{"description":{"identifier":"minecraft:air","protocol_id":0},"components":{}}}"#,
        r#"{"minecraft:item":{"description":{"identifier":"minecraft:stick","protocol_id":1},"components":{"minecraft:max_stack_size":64}}}"#,
        r#"{"minecraft:item":{"description":{"identifier":"minecraft:spear","protocol_id":2},"components":{"minecraft:max_stack_size":1,"minecraft:piercing_weapon":{},"minecraft:attack_animation":{"type":"stab","duration":12}}}}"#,
    ];
    let table = bake(
        files.iter().map(|source| DefinitionFile {
            name: "test",
            bytes: source.as_bytes(),
        }),
        &ItemTable::leak([]),
    )
    .unwrap()
    .table;
    let world = app.world_mut();
    world.insert_resource(CurrentRegistries::new(
        mcrs_minecraft_game_registries::registry_load::test_registries().clone(),
    ));
    world.insert_resource(table);
    world.insert_resource(BirthClock(100));
    app.add_plugins((
        TimePlugin,
        ItemPlugin::default(),
        LoadedClientPlugin,
        PlayerSwingPlugin,
        SwingPlugin,
    ))
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    app.update();
    app
}

fn spawn_player(world: &mut World, loaded: bool, mode: GameMode) -> Entity {
    let mut entity = world.spawn((
        Owner(PlayerSession(1)),
        PlayerGameMode(mode),
        PlayerSwing::default(),
        PlayerFileState::default(),
        Slots::new(41),
        Selected(0),
    ));
    if !loaded {
        entity.insert(UnloadedClient::default());
    }
    entity.id()
}

fn punch(world: &mut World, player: Entity) {
    let mut raw = Vec::new();
    ServerboundPunch.encode(&mut raw).unwrap();
    world.trigger(Received::<ServerboundPunch>::new(player, Bytes::from(raw)));
}

fn player_action(world: &mut World, player: Entity, action: PlayerAction) {
    let mut raw = Vec::new();
    ServerboundPlayerAction {
        action,
        pos: BlockPos::new(0, 64, 0),
        direction: Direction::Up,
        sequence: VarInt(0),
    }
    .encode(&mut raw)
    .unwrap();
    world.trigger(Received::<ServerboundPlayerAction>::new(
        player,
        Bytes::from(raw),
    ));
}

#[test]
fn punch_requires_loaded_alive_non_spectator_and_reads_attack_animation() {
    let mut app = test_app();
    let unloaded = spawn_player(app.world_mut(), false, GameMode::Survival);
    punch(app.world_mut(), unloaded);
    app.update();
    assert_eq!(
        app.world().get::<PlayerSwing>(unloaded),
        Some(&PlayerSwing::default()),
        "SWG-SRC-002: unloaded client punch is ignored"
    );

    let spectator = spawn_player(app.world_mut(), true, GameMode::Spectator);
    punch(app.world_mut(), spectator);
    app.update();
    assert_eq!(
        app.world().get::<PlayerSwing>(spectator),
        Some(&PlayerSwing::default()),
        "SWG-SRC-002: spectator punch is ignored"
    );

    let dead = spawn_player(app.world_mut(), true, GameMode::Survival);
    app.world_mut().entity_mut(dead).insert(Vitals {
        death_time: Some(0),
        ..Vitals::new(0.0)
    });
    punch(app.world_mut(), dead);
    app.update();
    assert_eq!(
        app.world().get::<PlayerSwing>(dead),
        Some(&PlayerSwing::default()),
        "SWG-SRC-002: dead player punch is ignored"
    );

    let alive = spawn_player(app.world_mut(), true, GameMode::Survival);
    let custom =
        ItemStack::new(STICK, Count::new(1).unwrap()).set(AttackAnimation(SwingAnimation {
            kind: SwingKind::Stab,
            duration: 10,
        }));
    app.world_mut()
        .commands()
        .spawn_stack(custom)
        .put_at(alive, 0);
    app.world_mut().flush();
    punch(app.world_mut(), alive);
    app.update();

    let tracker = app.world().get::<PlayerSwing>(alive).unwrap().0;
    assert!(
        !tracker.can_start(105),
        "SWG-CMP-002: punch reads attack_animation duration 10 (blocks T+5)"
    );
    assert!(
        tracker.can_start(106),
        "SWG-CMP-002: punch with duration 10 unlocks at T+6"
    );
}

#[test]
fn stab_requires_piercing_weapon_and_ignores_non_stab_actions() {
    let mut app = test_app();
    let player = spawn_player(app.world_mut(), true, GameMode::Survival);
    let stick = ItemStack::new(STICK, Count::new(1).unwrap());
    app.world_mut()
        .commands()
        .spawn_stack(stick)
        .put_at(player, 0);
    let offhand_spear = ItemStack::new(SPEAR, Count::new(1).unwrap());
    app.world_mut()
        .commands()
        .spawn_stack(offhand_spear)
        .put_at(player, 40);
    app.world_mut().flush();

    player_action(app.world_mut(), player, PlayerAction::Stab);
    app.update();
    assert_eq!(
        app.world().get::<PlayerSwing>(player),
        Some(&PlayerSwing::default()),
        "SWG-SRC-003: stab without main-hand piercing_weapon (even with off-hand spear) must emit no swing"
    );

    let spear = ItemStack::new(SPEAR, Count::new(1).unwrap());
    app.world_mut()
        .commands()
        .spawn_stack(spear)
        .put_at(player, 1);
    app.world_mut().flush();
    app.world_mut().get_mut::<Selected>(player).unwrap().0 = 1;

    for ignored in [
        PlayerAction::StartDestroyBlock,
        PlayerAction::AbortDestroyBlock,
        PlayerAction::StopDestroyBlock,
        PlayerAction::ReleaseUseItem,
        PlayerAction::SwapItemWithOffhand,
    ] {
        player_action(app.world_mut(), player, ignored);
    }
    app.update();
    assert_eq!(
        app.world().get::<PlayerSwing>(player),
        Some(&PlayerSwing::default()),
        "SWG-SRC-003: non-stab player actions emit no swing"
    );

    player_action(app.world_mut(), player, PlayerAction::Stab);
    app.update();
    let tracker = app.world().get::<PlayerSwing>(player).unwrap().0;
    assert!(
        !tracker.can_start(106),
        "SWG-SRC-003: spear stab duration 12 blocks T+6"
    );
    assert!(
        tracker.can_start(107),
        "SWG-SRC-003: spear stab duration 12 unlocks at T+7"
    );
}
