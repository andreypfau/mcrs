use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::serverbound::{PlayerCommandAction, ServerboundPlayerCommand};
use mcrs_minecraft_server::{VanillaEntityAttributesPlugin, VanillaSprintAttributePlugin};

use crate::host_app::{self, PlayServer};

#[test]
fn atr_src_002_the_sprint_state_moves_movement_speed_and_its_end_restores_it() {
    let mut app = host_app::make_host_app();
    app.add_plugins((VanillaEntityAttributesPlugin, VanillaSprintAttributePlugin));
    let mut server = PlayServer::start(app, &[], 0);
    let (anchor, _) = server.join(Uuid::from_u128(1), "Sprinter");
    let player = server.player(anchor);
    let entity_id = VarInt(player_network_id(server.session_of(anchor).unwrap()));
    let speed_after = |server: &mut PlayServer, action| {
        server.send(
            anchor,
            &ServerboundPlayerCommand {
                entity_id,
                action,
                data: VarInt(0),
            },
        );
        server
            .world()
            .get::<AttributeInstances>(player)
            .unwrap()
            .value(Attribute::MovementSpeed)
            .unwrap()
    };

    let sprinting = speed_after(&mut server, PlayerCommandAction::StartSprinting);
    let walking = speed_after(&mut server, PlayerCommandAction::StopSprinting);

    assert_eq!(
        sprinting.to_bits(),
        (f64::from(0.1_f32) * (1.0 + f64::from(0.3_f32))).to_bits()
    );
    assert_eq!(walking.to_bits(), f64::from(0.1_f32).to_bits());
}
