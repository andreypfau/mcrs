use std::path::Path;

use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_container_click::{Click, ClickKind, Clicked};
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine::bus::InboundPlayerDespawn;
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::persisted_registry::{PersistedRegistry, PersistedSnapshot};
use mcrs_minecraft_engine_item::{Dropped, Slots};
use mcrs_minecraft_engine_registries::{install_item_table, test_item_table};
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{Count, Kind};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, VarInt};
use mcrs_minecraft_protocol_game::serverbound::{
    ClientCommandAction, ServerboundChatCommand, ServerboundClientCommand,
    ServerboundContainerClick,
};
use mcrs_minecraft_save::PlayerDat;
use mcrs_minecraft_server::{
    VanillaDeathDropsPlugin, VanillaEntityAttributesPlugin, VanillaEntityPhysicsPlugin,
    VanillaItemEntityPlugin, VanillaItemPlugin, VanillaRespawnPlugin,
};

use crate::commands::load_stone_column;
use crate::host_app::{self, Arrival, PlayServer};

type Held = (Item, u8);

const HEAD: u16 = 39;
const OFF_HAND: u16 = 40;
const SPECTATOR: i32 = 3;
const MENU_SLOT_OF_MAIN_3: i16 = 39;
const STONE: Held = (Item::Stone, 5);
const HELMET: Held = (Item::IronHelmet, 1);
const TORCH: Held = (Item::Torch, 7);
const START: [(u16, Held); 3] = [(3, STONE), (HEAD, HELMET), (OFF_HAND, TORCH)];

#[derive(Clone, Copy)]
enum Exit {
    Respawn,
    Leave,
}

struct Case {
    name: &'static str,
    keep_inventory: bool,
    game_type: i32,
    helmet_enchantment: Option<&'static str>,
    cursor_from: Option<i16>,
    held: &'static [(u16, Held)],
    before_removal: Option<Exit>,
    dropped_at_death: &'static [Held],
    dropped_at_removal: &'static [Held],
}

fn stack(item: Item, count: u8) -> NbtCompound {
    let mut stack = NbtCompound::new();
    stack.put_string("id", item.as_static_str().to_owned());
    stack.put_int("count", i32::from(count));
    stack
}

fn file(case: &Case) -> PlayerDat {
    let mut main = stack(STONE.0, STONE.1);
    main.put_byte("Slot", 3);
    let mut helmet = stack(HELMET.0, HELMET.1);
    if let Some(enchantment) = case.helmet_enchantment {
        let mut levels = NbtCompound::new();
        levels.put_int(enchantment, 1);
        let mut components = NbtCompound::new();
        components.put("minecraft:enchantments", levels);
        helmet.put("components", components);
    }
    let mut equipment = NbtCompound::new();
    equipment.put("head", helmet);
    equipment.put("offhand", stack(TORCH.0, TORCH.1));
    let mut dat = PlayerDat::default();
    dat.root.put_list("Inventory", vec![NbtTag::Compound(main)]);
    dat.root.put("equipment", equipment);
    dat.root.put_int("playerGameType", case.game_type);
    dat
}

fn held(world: &World, player: Entity) -> Vec<(u16, Held)> {
    let slots = world.get::<Slots>(player).unwrap();
    (0..)
        .zip(slots.iter())
        .filter_map(|(slot, stack)| Some((slot, read(world, stack?))))
        .collect()
}

fn read(world: &World, stack: Entity) -> Held {
    let stack = world.entity(stack);
    let kind = stack.get::<Kind>().unwrap();
    let item = Item::from_protocol_id(kind.0).unwrap();
    (item, stack.get::<Count>().unwrap().get())
}

fn dropped(world: &mut World) -> Vec<Held> {
    let stacks: Vec<Entity> = world
        .query_filtered::<Entity, With<Dropped>>()
        .iter(world)
        .collect();
    let mut dropped: Vec<Held> = stacks.into_iter().map(|stack| read(world, stack)).collect();
    dropped.sort_unstable();
    dropped
}

const CASES: [Case; 10] = [
    Case {
        name: "PLD-DRP-001: the inventory drops",
        keep_inventory: false,
        game_type: 0,
        helmet_enchantment: None,
        cursor_from: None,
        held: &[],
        before_removal: None,
        dropped_at_death: &[STONE, TORCH, HELMET],
        dropped_at_removal: &[STONE, TORCH, HELMET],
    },
    Case {
        name: "PLD-DRP-001: keep_inventory keeps the inventory",
        keep_inventory: true,
        game_type: 0,
        helmet_enchantment: None,
        cursor_from: None,
        held: &START,
        before_removal: None,
        dropped_at_death: &[],
        dropped_at_removal: &[],
    },
    Case {
        name: "PLD-DRP-002: a vanishing stack is deleted",
        keep_inventory: false,
        game_type: 0,
        helmet_enchantment: Some("minecraft:vanishing_curse"),
        cursor_from: None,
        held: &[],
        before_removal: None,
        dropped_at_death: &[STONE, TORCH],
        dropped_at_removal: &[STONE, TORCH],
    },
    Case {
        name: "PLD-DRP-002: keep_inventory keeps a vanishing stack",
        keep_inventory: true,
        game_type: 0,
        helmet_enchantment: Some("minecraft:vanishing_curse"),
        cursor_from: None,
        held: &START,
        before_removal: None,
        dropped_at_death: &[],
        dropped_at_removal: &[],
    },
    Case {
        name: "PLD-DRP-006: a spectator drops nothing",
        keep_inventory: false,
        game_type: SPECTATOR,
        helmet_enchantment: None,
        cursor_from: None,
        held: &START,
        before_removal: None,
        dropped_at_death: &[],
        dropped_at_removal: &[],
    },
    Case {
        name: "PLD-DRP-006: a spectator keeps a vanishing stack",
        keep_inventory: false,
        game_type: SPECTATOR,
        helmet_enchantment: Some("minecraft:vanishing_curse"),
        cursor_from: None,
        held: &START,
        before_removal: None,
        dropped_at_death: &[],
        dropped_at_removal: &[],
    },
    Case {
        name: "PLD-DRP-005: the cursor drops once at the removal",
        keep_inventory: false,
        game_type: 0,
        helmet_enchantment: None,
        cursor_from: Some(MENU_SLOT_OF_MAIN_3),
        held: &[],
        before_removal: None,
        dropped_at_death: &[TORCH, HELMET],
        dropped_at_removal: &[STONE, TORCH, HELMET],
    },
    Case {
        name: "PLD-DRP-005: the cursor drops also with keep_inventory",
        keep_inventory: true,
        game_type: 0,
        helmet_enchantment: None,
        cursor_from: Some(MENU_SLOT_OF_MAIN_3),
        held: &[(HEAD, HELMET), (OFF_HAND, TORCH)],
        before_removal: None,
        dropped_at_death: &[],
        dropped_at_removal: &[STONE],
    },
    Case {
        name: "PLD-DRP-005: the cursor drops once at a respawn before the removal",
        keep_inventory: false,
        game_type: 0,
        helmet_enchantment: None,
        cursor_from: Some(MENU_SLOT_OF_MAIN_3),
        held: &[],
        before_removal: Some(Exit::Respawn),
        dropped_at_death: &[TORCH, HELMET],
        dropped_at_removal: &[STONE, TORCH, HELMET],
    },
    Case {
        name: "PLD-DRP-005: the cursor drops once when the dead player leaves",
        keep_inventory: false,
        game_type: 0,
        helmet_enchantment: None,
        cursor_from: Some(MENU_SLOT_OF_MAIN_3),
        held: &[],
        before_removal: Some(Exit::Leave),
        dropped_at_death: &[TORCH, HELMET],
        dropped_at_removal: &[STONE, TORCH, HELMET],
    },
];

fn sorted(stacks: &[Held]) -> Vec<Held> {
    let mut stacks = stacks.to_vec();
    stacks.sort_unstable();
    stacks
}

fn pick_up(server: &mut PlayServer, anchor: Entity, slot: i16) {
    server.queue(
        anchor,
        &ServerboundContainerClick {
            container_id: VarInt(0),
            state_seqno: VarInt(0),
            click: Click {
                kind: ClickKind::PickUp,
                button: 0,
                clicked: Clicked::from(slot),
            },
            changed_slots: Vec::new().into(),
            carried_item: None,
        },
    );
    server.ticks(anchor, 2);
}

fn leave_death(server: &mut PlayServer, anchor: Entity, exit: Exit) {
    match exit {
        Exit::Respawn => server.queue(
            anchor,
            &ServerboundClientCommand {
                action: ClientCommandAction::PerformRespawn,
            },
        ),
        Exit::Leave => {
            let session = server.session_of(anchor).unwrap();
            let dim = server.dim;
            server.submit_to(dim, ToDim::Despawn(InboundPlayerDespawn { session }));
        }
    }
    server.ticks(anchor, 2);
}

#[test]
fn a_dying_player_drops_what_the_rules_select() {
    for case in &CASES {
        let mut app = host_app::make_host_app();
        install_item_table(app.world_mut(), test_item_table()).unwrap();
        app.add_plugins((
            CommandPlugin::<VanillaCommands>::new(),
            VanillaCommandsPlugin,
            VanillaEntityPhysicsPlugin,
            VanillaEntityAttributesPlugin,
            VanillaItemEntityPlugin,
            VanillaItemPlugin,
            VanillaDeathDropsPlugin,
            VanillaRespawnPlugin,
        ));
        let mut server = PlayServer::start_in(
            app,
            &[("minecraft:overworld", "minecraft:overworld")],
            &[],
            2,
        );
        load_stone_column(&mut server);
        server
            .world()
            .resource_mut::<GameRules>()
            .set(GameRule::KeepInventory, i32::from(case.keep_inventory));
        let registry = server.app.world().resource::<PersistedRegistry>();
        let snapshot =
            PersistedSnapshot::from_dat(&file(case), registry, Path::new("player.dat")).unwrap();
        let arrival = Arrival {
            uuid: Uuid::new_v4(),
            name: "dying",
            position: DVec3::new(8.5, 64.0, 8.5),
            view_distance: 2,
        };
        let anchor = host_app::admit_with_file(&mut server.app, server.dim, &arrival, &snapshot);
        server.ticks(anchor, 3);
        let player = server.player(anchor);
        assert_eq!(
            held(server.world(), player),
            START,
            "{}: the file fills the slots",
            case.name
        );
        if let Some(slot) = case.cursor_from {
            pick_up(&mut server, anchor, slot);
        }

        server.queue(
            anchor,
            &ServerboundChatCommand {
                command: Bounded("kill @s"),
            },
        );
        server.ticks(anchor, 2);

        let world = server.world();
        assert!(
            !world.get::<Vitals>(player).unwrap().is_alive(),
            "{}: the player is dead",
            case.name
        );
        assert_eq!(
            (held(world, player), dropped(world)),
            (case.held.to_vec(), sorted(case.dropped_at_death)),
            "{}: at the death",
            case.name
        );
        if let Some(exit) = case.before_removal {
            leave_death(&mut server, anchor, exit);
            assert_eq!(
                dropped(server.world()),
                sorted(case.dropped_at_removal),
                "{}: before the removal",
                case.name
            );
        }
        server.ticks(anchor, 22);
        assert_eq!(
            dropped(server.world()),
            sorted(case.dropped_at_removal),
            "{}: after the removal",
            case.name
        );
    }
}
