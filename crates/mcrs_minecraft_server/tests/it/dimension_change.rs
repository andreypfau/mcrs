use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_math::DVec3;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::physics::{Rotation, Transform};
use mcrs_minecraft_level::entity::player::{ChangeDimensionRequest, Player};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_server::world::entity::player::HostAnchor;

use crate::host_app::{self, PlayServer};

fn player_in(server: &mut PlayServer, dim: Entity, anchor: Entity) -> Option<(Entity, Transform)> {
    let world = server.world_in(dim);
    world
        .query_filtered::<(Entity, &HostAnchor, &Transform), With<Player>>()
        .iter(world)
        .find(|(_, host, _)| host.0 == anchor)
        .map(|(entity, _, transform)| (entity, *transform))
}

#[test]
fn a_request_written_in_a_dimension_moves_the_player_with_the_packets_of_the_change() {
    let mut server = PlayServer::start_in(
        host_app::make_host_app(),
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_end", "minecraft:the_end"),
        ],
        &[],
        0,
    );
    let (anchor, _) = server.join(Uuid::new_v4(), "Walker");
    let overworld = server.dim;
    let end = host_app::label_of(&server.app, "minecraft:the_end");
    let (player, _) = player_in(&mut server, overworld, anchor).unwrap();
    let transform = Transform {
        translation: DVec3::new(100.5, 49.0, 0.5),
        rotation: Rotation::new(90.0, 0.0),
    };
    server
        .world()
        .resource_mut::<Messages<ChangeDimensionRequest>>()
        .write(ChangeDimensionRequest {
            player,
            to: ResourceKey::from_location(ResourceLocation::read("minecraft:the_end").unwrap()),
            transform,
        });

    let packets = server.ticks(anchor, 1);

    assert!(player_in(&mut server, overworld, anchor).is_none());
    let (arrived, at) = player_in(&mut server, end, anchor).unwrap();
    assert_eq!(at, transform);
    assert_eq!(
        server
            .world_in(end)
            .get::<GameProfile>(arrived)
            .map(|profile| profile.username.as_str()),
        Some("Walker"),
        "the player takes its registered components"
    );
    let start = packets
        .iter()
        .position(|packet| matches!(packet, PacketPayload::Respawn(_)))
        .unwrap_or_else(|| panic!("{packets:?}"));
    let PacketPayload::Respawn(respawn) = &packets[start] else {
        unreachable!()
    };
    assert_eq!(
        respawn.player_spawn_info.dimension.as_str(),
        "minecraft:the_end"
    );
    let position = packets[start..]
        .iter()
        .find_map(|packet| match packet {
            PacketPayload::PlayerPosition(position) => Some(position),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{packets:?}"));
    assert_eq!(position.position, transform.translation);
    assert_eq!(position.look.yaw, 90.0);
}
