//! An entity that faces a real angle must reach the outbound payload with that
//! angle. The pair of raw quaternion components this used to read produced a
//! `[-1, 1]` number in a field the wire encoder treats as degrees, so every
//! synced entity arrived facing roughly zero.

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_engine::entity::physics::{OldTransform, Rotation, Transform};
use mcrs_engine::entity::EntityNetworkSyncEvent;
use mcrs_minecraft::world::bus::{OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft::world::entity::entity_pos_sync;

#[test]
fn a_synced_entity_keeps_its_look_angles_on_the_wire() {
    let mut app = App::new();
    app.add_message::<OutboundPlayerPacket>();
    app.add_observer(entity_pos_sync);

    let transform = Transform::from_translation(DVec3::new(1.0, 64.0, 2.0))
        .with_rotation(Rotation::new(90.0, -45.0));
    let entity = app
        .world_mut()
        .spawn((transform, OldTransform(transform)))
        .id();
    let player = app.world_mut().spawn_empty().id();

    app.world_mut()
        .trigger(EntityNetworkSyncEvent { entity, player });

    let sent: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .collect();
    let [packet] = sent.as_slice() else {
        panic!("expected exactly one outbound packet, got {}", sent.len());
    };
    let PacketPayload::EntityPosSync { look, .. } = &packet.data else {
        panic!("expected an EntityPosSync payload, got {:?}", packet.data);
    };
    assert_eq!(look.yaw, 90.0);
    assert_eq!(look.pitch, -45.0);
}
