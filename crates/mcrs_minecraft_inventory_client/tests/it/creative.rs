use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_ecs::world::World;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_engine_item::Denial;
use mcrs_minecraft_engine_item::ItemAppExt;
use mcrs_minecraft_engine_item::RequestCheck;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_inventory_client::CreativeArea;
use mcrs_minecraft_inventory_client::CreativeClick;
use mcrs_minecraft_inventory_client::MenuAction;
use mcrs_minecraft_inventory_client::MenuInput;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::WireStackWithLength;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_network::client::ClientConnection;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCreativeModeSlot;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::fixture::STONE;
use crate::fixture::client_with_player;
use crate::fixture::cursor;
use crate::fixture::stack;
use crate::server::Server;

const BURST: usize = 64;

#[derive(Resource, Clone, Copy)]
struct DenyEach(Denial);

fn deny_each(mut check: On<RequestCheck>, denial: Res<DenyEach>) {
    check.deny(denial.0);
}

fn creative(area: CreativeArea, kind: ClickKind) -> MenuAction {
    MenuAction::Creative(CreativeClick {
        area,
        click: Click {
            kind,
            button: 0,
            clicked: Clicked::Outside,
        },
    })
}

fn with_length(world: &World, stack: &ItemStack) -> WireStackWithLength {
    let names = RegistryNames::new(
        world.resource::<CurrentRegistries>(),
        world.resource::<ItemTable>(),
    );
    WireStackWithLength::write(Some(stack), &names).expect("the stack has a wire form")
}

#[test]
fn the_drop_counter_passes_a_burst_of_creative_drops_and_refuses_the_next() {
    let (mut app, player) = client_with_player();
    let mut server = Server::join(&mut app);
    app.world_mut()
        .entity_mut(player)
        .insert(Abilities::new(GameMode::Creative));
    let shown = stack(STONE, 1);
    let throw = MenuAction::Creative(CreativeClick {
        area: CreativeArea::Grid(Some(shown.clone())),
        click: Click {
            kind: ClickKind::Throw,
            button: 0,
            clicked: Clicked::Outside,
        },
    });
    for _ in 0..=BURST {
        app.world_mut().write_message(MenuInput {
            viewer: player,
            action: throw.clone(),
        });
    }
    app.update();
    app.world_mut()
        .query::<&mut ClientConnection>()
        .single_mut(app.world_mut())
        .expect("one connection")
        .write_packet(&ServerboundChatCommand {
            command: Bounded("marker"),
        });
    app.update();

    let drop = ServerboundSetCreativeModeSlot {
        slot: -1,
        item: with_length(app.world(), &shown),
    };
    let sent =
        server.each_before::<ServerboundSetCreativeModeSlot, ServerboundChatCommand<'static>>();
    assert_eq!(sent, vec![drop; BURST]);
}

#[test]
fn a_denied_creative_drop_sends_no_packet_and_keeps_the_cursor() {
    let (mut app, player) = client_with_player();
    let mut server = Server::join(&mut app);
    let denial = Denial {
        source: app.allocate_request_source().unwrap(),
        code: 1,
    };
    app.insert_resource(DenyEach(denial))
        .add_observer(deny_each);
    app.world_mut()
        .entity_mut(player)
        .insert(Abilities::new(GameMode::Creative));
    let shown = stack(STONE, 1);
    for action in [
        creative(CreativeArea::Grid(Some(shown)), ClickKind::PickUp),
        creative(CreativeArea::Inventory, ClickKind::PickUp),
    ] {
        app.world_mut().write_message(MenuInput {
            viewer: player,
            action,
        });
    }
    app.update();
    app.world_mut()
        .query::<&mut ClientConnection>()
        .single_mut(app.world_mut())
        .expect("one connection")
        .write_packet(&ServerboundChatCommand {
            command: Bounded("marker"),
        });
    app.update();

    let sent =
        server.each_before::<ServerboundSetCreativeModeSlot, ServerboundChatCommand<'static>>();
    assert_eq!(sent, vec![]);
    assert_eq!(cursor(app.world()), Some((STONE, 1)));
}
