use bevy_app::App;
use bevy_app::FixedUpdate;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_inventory_client::HandInput;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_player_holders::HandAction;
use mcrs_minecraft_protocol_game::Direction;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetPlayerInventory;
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerAction;
use mcrs_minecraft_wire::VarInt;
use mcrs_voxel::BlockPos;

use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::body;
use crate::fixture::client_with_player;
use crate::fixture::content;
use crate::fixture::stack;
use crate::fixture::wire;
use crate::server::Server;

type Held = Option<(Kind, u8)>;

struct Row {
    name: &'static str,
    inventory: &'static [(i32, (Kind, u8))],
    game_mode: GameMode,
    action: HandAction,
    main_hand: Held,
    off_hand: Held,
    sent: Option<PlayerAction>,
}

const STONES: (Kind, u8) = (STONE, 10);
const DIRTS: (Kind, u8) = (DIRT, 3);
const MAIN_HAND_ENTRY: u16 = 36;
const OFF_HAND_ENTRY: u16 = 45;

const ROWS: &[Row] = &[
    Row {
        name: "a drop of one item leaves the rest in the hand",
        inventory: &[(0, STONES)],
        game_mode: GameMode::Survival,
        action: HandAction::DropOne,
        main_hand: Some((STONE, 9)),
        off_hand: None,
        sent: Some(PlayerAction::DropItem),
    },
    Row {
        name: "a drop of the stack empties the hand",
        inventory: &[(0, STONES)],
        game_mode: GameMode::Survival,
        action: HandAction::DropStack,
        main_hand: None,
        off_hand: None,
        sent: Some(PlayerAction::DropAllItems),
    },
    Row {
        name: "a drop from an empty hand changes nothing and is sent",
        inventory: &[(40, DIRTS)],
        game_mode: GameMode::Survival,
        action: HandAction::DropOne,
        main_hand: None,
        off_hand: Some(DIRTS),
        sent: Some(PlayerAction::DropItem),
    },
    Row {
        name: "a hand swap changes no slot and is sent",
        inventory: &[(0, STONES), (40, DIRTS)],
        game_mode: GameMode::Survival,
        action: HandAction::SwapHands,
        main_hand: Some(STONES),
        off_hand: Some(DIRTS),
        sent: Some(PlayerAction::SwapItemWithOffhand),
    },
    Row {
        name: "a spectator drops nothing and sends nothing",
        inventory: &[(0, STONES)],
        game_mode: GameMode::Spectator,
        action: HandAction::DropStack,
        main_hand: Some(STONES),
        off_hand: None,
        sent: None,
    },
];

fn press(app: &mut App, input: HandInput) {
    app.world_mut().write_message(input);
    app.world_mut().run_schedule(FixedUpdate);
    app.update();
}

fn assert_sent(packet: &ServerboundPlayerAction, action: PlayerAction, name: &str) {
    assert_eq!(packet.action, action, "{name}: action");
    assert_eq!(packet.pos, BlockPos::new(0, 0, 0), "{name}: position");
    assert_eq!(packet.direction, Direction::Down, "{name}: face");
    assert_eq!(packet.sequence.0, 0, "{name}: sequence");
}

#[test]
fn a_hand_key_is_predicted_and_sent() {
    for row in ROWS {
        let (mut app, player) = client_with_player();
        let mut server = Server::join(&mut app);
        let world = app.world_mut();
        let connection = world.spawn_empty().id();
        for &(slot, (kind, count)) in row.inventory {
            let contents = wire(world, Some(&stack(kind, count)));
            let body = body(&ClientboundSetPlayerInventory {
                slot: VarInt(slot),
                contents,
            });
            world.trigger(Received::<ClientboundSetPlayerInventory>::new(
                connection,
                body.into(),
            ));
        }
        world
            .entity_mut(player)
            .insert(PlayerGameMode(row.game_mode));
        world.flush();
        press(
            &mut app,
            HandInput {
                viewer: player,
                action: row.action,
            },
        );
        let world = app.world();
        assert_eq!(
            content(world, MAIN_HAND_ENTRY),
            row.main_hand,
            "{}: main hand",
            row.name
        );
        assert_eq!(
            content(world, OFF_HAND_ENTRY),
            row.off_hand,
            "{}: off hand",
            row.name
        );
        app.world_mut()
            .entity_mut(player)
            .insert(PlayerGameMode(GameMode::Survival));
        press(
            &mut app,
            HandInput {
                viewer: player,
                action: HandAction::SwapHands,
            },
        );
        if let Some(action) = row.sent {
            assert_sent(&server.next::<ServerboundPlayerAction>(), action, row.name);
        }
        let marker = server.next::<ServerboundPlayerAction>();
        assert_sent(&marker, PlayerAction::SwapItemWithOffhand, row.name);
    }
}
