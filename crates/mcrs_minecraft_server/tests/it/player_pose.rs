use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, Packet};
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetEntityData;
use mcrs_minecraft_protocol_game::entity::{MetaDataValue, MetadataEntry};
use mcrs_minecraft_protocol_game::serverbound::{PlayerInputFlags, ServerboundPlayerInput};
use mcrs_minecraft_server::{VanillaEntityPhysicsPlugin, VanillaPlayerPosePlugin};

use crate::commands::load_stone_column;
use crate::host_app::{self, PlayServer, replicated, sent_to};

#[test]
fn pose_net_003_a_shift_input_reaches_each_tracker_as_crouching_and_the_shift_flag() {
    let mut app = host_app::make_host_app();
    app.add_plugins((VanillaEntityPhysicsPlugin, VanillaPlayerPosePlugin));
    let mut server = PlayServer::start(app, &[], 0);
    load_stone_column(&mut server);
    let dim = server.dim;
    let shifter = server.admit(dim, Uuid::from_u128(1), "Shifter");
    let watcher = server.admit(dim, Uuid::from_u128(2), "Watcher");
    server.ticks_all(3);
    let shifter_id = player_network_id(server.session_of(shifter).unwrap());
    let watcher_session = server.session_of(watcher).unwrap();

    server.queue(
        shifter,
        &ServerboundPlayerInput {
            input: PlayerInputFlags::new().with_shift(true),
        },
    );
    let packets = sent_to(watcher_session, server.ticks_all(2));

    let records = replicated(&packets);
    let entries: Vec<MetadataEntry<'_>> = records
        .iter()
        .filter(|(id, _)| *id == ClientboundSetEntityData::ID)
        .map(|(_, bytes)| ClientboundSetEntityData::decode(&mut bytes.as_slice()).unwrap())
        .filter(|data| data.entity_id.0 == shifter_id)
        .flat_map(|data| data.metadata.0)
        .collect();
    assert_eq!(
        entries,
        [
            MetadataEntry {
                index: 0,
                value: MetaDataValue::Byte(0x02),
            },
            MetadataEntry {
                index: 6,
                value: MetaDataValue::Pose(Pose::Crouching),
            },
        ]
    );
}

#[test]
fn phy_box_006_a_crawling_player_with_free_space_above_stands_up_without_a_new_input() {
    let mut app = host_app::make_host_app();
    app.add_plugins((VanillaEntityPhysicsPlugin, VanillaPlayerPosePlugin));
    let mut server = PlayServer::start(app, &[], 0);
    load_stone_column(&mut server);
    let dim = server.dim;
    let anchor = server.admit(dim, Uuid::from_u128(1), "Crawler");
    server.ticks_all(3);
    let player = server.player(anchor);
    *server.world().get_mut::<Pose>(player).unwrap() = Pose::Swimming;

    server.ticks_all(1);

    assert_eq!(server.world().get::<Pose>(player), Some(&Pose::Standing));
}
