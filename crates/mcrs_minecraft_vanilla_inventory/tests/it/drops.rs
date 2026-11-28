use bevy_app::FixedUpdate;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::ItemAppExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_vanilla_inventory::RequestPurpose;
use mcrs_minecraft_vanilla_inventory::VanillaInventoryRequests;

use crate::fixture::STONE;
use crate::fixture::dropped;
use crate::fixture::gameplay_app;
use crate::fixture::put;

#[test]
fn only_a_drop_of_the_vanilla_source_launches_the_stack() {
    let cases = [
        ("the vanilla source", true, vec![(STONE, 2, 40)]),
        ("another source", false, Vec::new()),
    ];
    for (label, vanilla, expected) in cases {
        let mut app = gameplay_app();
        let other = app
            .allocate_request_source()
            .expect("a free request source");
        let world = app.world_mut();
        let player = world.spawn(Player).id();
        world.flush();
        put(world, player, 0, STONE, 2);
        let source = if vanilla {
            world.resource::<VanillaInventoryRequests>().source
        } else {
            other
        };
        let from = SlotRef {
            holder: player,
            index: 0,
        };

        world.commands().entity(player).request(
            Request::new(RequestId {
                source,
                serial: RequestPurpose::Click.serial(),
            })
            .drop(from, 2),
        );
        world.flush();
        world.run_schedule(FixedUpdate);

        assert_eq!(dropped(world), expected, "{label}");
    }
}
