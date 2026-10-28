use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_math::DVec3;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget, to};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::physics::{Rotation, Transform};
use mcrs_minecraft_level::entity::player::ChangeDimensionRequest;
use mcrs_minecraft_level::session::{Place, SessionPlacement};
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::game_event::GameEventKind;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundChunkCacheRadius;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_server::world::entity::player::ability::PlayerFlying;
use mcrs_minecraft_server::world::entity::player::dimension_change;

use crate::host_app::{self, PlayServer};

fn player_in(server: &mut PlayServer, dim: Entity, anchor: Entity) -> Option<(Entity, Transform)> {
    let player = server.player_in(dim, anchor)?;
    let transform = *server.world_in(dim).get::<Transform>(player)?;
    Some((player, transform))
}

/// A server with the Overworld and the End, and the player `Walker` in the Overworld: the
/// server, the anchor of the player, the Overworld and the End.
fn walker_in_the_overworld() -> (PlayServer, Entity, Entity, Entity) {
    let (server, anchor, overworld, end, _) = walker_and_its_join();
    (server, anchor, overworld, end)
}

/// As [`walker_in_the_overworld`], with the packets the player received on joining.
fn walker_and_its_join() -> (PlayServer, Entity, Entity, Entity, Vec<PacketPayload>) {
    let mut server = PlayServer::start_in(
        host_app::make_host_app(),
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_end", "minecraft:the_end"),
        ],
        &[],
        0,
    );
    let (anchor, joined) = server.join(Uuid::new_v4(), "Walker");
    let overworld = server.dim;
    let end = host_app::label_of(&server.app, "minecraft:the_end");
    (server, anchor, overworld, end, joined)
}

fn epoch_of(server: &PlayServer, anchor: Entity) -> u32 {
    server
        .app
        .world()
        .get::<SessionPlacement>(anchor)
        .unwrap()
        .epoch()
}

fn position_of(packets: &[PacketPayload], wanted: impl Fn(&PacketPayload) -> bool) -> usize {
    packets
        .iter()
        .position(wanted)
        .unwrap_or_else(|| panic!("{packets:?}"))
}

fn the_end() -> ResourceKey<mcrs_minecraft_dimension::Dimension> {
    ResourceKey::from_location(ResourceLocation::read("minecraft:the_end").unwrap())
}

#[test]
fn a_request_written_in_a_dimension_moves_the_player_with_the_packets_of_the_change() {
    let (mut server, anchor, overworld, end, joined) = walker_and_its_join();
    let network_id = joined
        .iter()
        .find_map(|packet| match packet {
            PacketPayload::PlayerLogin(login) => Some(login.player_id),
            _ => None,
        })
        .unwrap();
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
            to: the_end(),
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
    let respawn = position_of(&packets, |packet| {
        matches!(packet, PacketPayload::Respawn(_))
    });
    let PacketPayload::Respawn(sent) = &packets[respawn] else {
        unreachable!()
    };
    assert_eq!(
        sent.player_spawn_info.dimension.as_str(),
        "minecraft:the_end"
    );
    assert_eq!(sent.data_to_keep, 3);
    let events: Vec<usize> = packets
        .iter()
        .enumerate()
        .filter(|(_, packet)| matches!(packet, PacketPayload::OpLevelEntityEvent(_)))
        .map(|(at, _)| at)
        .collect();
    let [event] = events[..] else {
        panic!("the arrival sends the level once: {packets:?}");
    };
    let PacketPayload::OpLevelEntityEvent(level) = &packets[event] else {
        unreachable!()
    };
    assert_eq!(
        level.entity_id, network_id,
        "the player keeps its network id"
    );
    let moved = position_of(&packets, |packet| {
        matches!(packet, PacketPayload::PlayerPosition(_))
    });
    let PacketPayload::PlayerPosition(position) = &packets[moved] else {
        unreachable!()
    };
    assert_eq!(position.position, transform.translation);
    assert_eq!(position.look.yaw, 90.0);
    let loading = position_of(&packets, |packet| {
        matches!(packet, PacketPayload::GameEvent(event)
            if matches!(event.game_event, GameEventKind::LevelChunksLoadStart))
    });
    assert!(
        respawn < event && event < moved && moved < loading,
        "respawn {respawn}, level {event}, position {moved}, chunk load start {loading}"
    );
}

#[test]
fn packets_the_old_dimension_still_holds_leave_before_the_respawn_under_the_old_epoch() {
    let (mut server, anchor, overworld, _) = walker_in_the_overworld();
    let (player, transform) = player_in(&mut server, overworld, anchor).unwrap();
    let epoch = epoch_of(&server, anchor);
    let world = server.world_in(overworld);
    world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write(to(
            anchor,
            PacketPayload::SetChunkCacheRadius(ClientboundChunkCacheRadius { radius: VarInt(31) }),
        ));
    world
        .resource_mut::<Messages<ChangeDimensionRequest>>()
        .write(ChangeDimensionRequest {
            player,
            to: the_end(),
            transform,
        });

    dimension_change::apply_dimension_change_requests(&mut server.app);

    let sent: Vec<OutboundPlayerPacket> = server
        .app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .collect();
    let held = sent
        .iter()
        .position(|packet| {
            matches!(
                &packet.data,
                PacketPayload::SetChunkCacheRadius(radius) if radius.radius.0 == 31
            )
        })
        .unwrap_or_else(|| panic!("the held packet left with the change: {sent:?}"));
    let respawn = sent
        .iter()
        .position(|packet| matches!(packet.data, PacketPayload::Respawn(_)))
        .unwrap();
    assert!(held < respawn);
    assert_eq!(sent[held].epoch, epoch);

    let arrived = epoch_of(&server, anchor);
    assert_ne!(arrived, epoch);
    let later = server.ticks_all(2);
    let to_mover = |packet: &&OutboundPlayerPacket| matches!(packet.target, PacketTarget::SinglePlayer(target) if target == anchor);
    let epochs: Vec<u32> = sent[respawn..]
        .iter()
        .chain(&later)
        .filter(to_mover)
        .map(|packet| packet.epoch)
        .collect();
    assert!(
        !epochs.is_empty() && epochs.iter().all(|&at| at == arrived),
        "the respawn and every later packet carry the new epoch {arrived}: {epochs:?}"
    );
}

#[test]
fn a_change_the_player_cannot_complete_leaves_it_where_it_is() {
    let unplace = |server: &mut PlayServer, anchor: Entity, _: Entity| {
        server
            .app
            .world_mut()
            .get_mut::<SessionPlacement>(anchor)
            .unwrap()
            .set(Place::Unplaced);
    };
    let strip = |server: &mut PlayServer, _: Entity, player: Entity| {
        let dim = server.dim;
        server
            .world_in(dim)
            .entity_mut(player)
            .remove::<PlayerFlying>();
    };
    let cases: [(&str, &dyn Fn(&mut PlayServer, Entity, Entity)); 2] = [
        ("disconnected", &unplace),
        ("without its arrival state", &strip),
    ];
    for (case, prepare) in cases {
        let (mut server, anchor, overworld, end) = walker_in_the_overworld();
        let (player, transform) = player_in(&mut server, overworld, anchor).unwrap();
        prepare(&mut server, anchor, player);
        server
            .world_in(overworld)
            .resource_mut::<Messages<ChangeDimensionRequest>>()
            .write(ChangeDimensionRequest {
                player,
                to: the_end(),
                transform,
            });

        dimension_change::apply_dimension_change_requests(&mut server.app);
        let mut sent: Vec<PacketPayload> = server
            .app
            .world_mut()
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .map(|packet| packet.data)
            .collect();
        sent.extend(server.ticks(anchor, 1));

        assert!(player_in(&mut server, end, anchor).is_none(), "{case}");
        assert!(
            player_in(&mut server, overworld, anchor).is_some(),
            "{case}"
        );
        assert!(
            !sent.iter().any(|packet| matches!(
                packet,
                PacketPayload::Respawn(_) | PacketPayload::PlayerPosition(_)
            )),
            "{case}: {sent:?}"
        );
    }
}

#[test]
fn an_observer_in_the_new_dimension_sees_the_mover_with_its_network_id_and_no_list_update() {
    let (mut server, anchor, overworld, end, joined) = walker_and_its_join();
    let walker = server.app.world().get::<GameProfile>(anchor).unwrap().id;
    let network_id = joined
        .iter()
        .find_map(|packet| match packet {
            PacketPayload::PlayerLogin(login) => Some(login.player_id),
            _ => None,
        })
        .unwrap();
    let (greeter, _) = server.join_in(end, Uuid::new_v4(), "Greeter");
    let (player, transform) = player_in(&mut server, overworld, anchor).unwrap();
    server
        .world_in(overworld)
        .resource_mut::<Messages<ChangeDimensionRequest>>()
        .write(ChangeDimensionRequest {
            player,
            to: the_end(),
            transform,
        });

    let packets = server.ticks_all(3);

    let entered: Vec<VarInt> = packets
        .iter()
        .filter(|packet| {
            matches!(packet.target, PacketTarget::SinglePlayer(target) if target == greeter)
        })
        .filter_map(|packet| match &packet.data {
            PacketPayload::PlayerEnteredView(entity) if entity.uuid == walker => Some(entity.id),
            _ => None,
        })
        .collect();
    assert_eq!(
        entered,
        [VarInt(network_id)],
        "the walker keeps its network id"
    );
    assert!(
        !packets
            .iter()
            .any(|packet| matches!(packet.data, PacketPayload::PlayerInfoUpdate { .. })),
        "the tab list does not change"
    );
}
