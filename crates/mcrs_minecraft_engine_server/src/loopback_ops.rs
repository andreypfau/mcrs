use bevy_ecs::lifecycle::Insert;
use bevy_ecs::prelude::{On, Query, Res, ResMut, Resource};
use mcrs_minecraft_engine::coordinator::ops::{OpList, OpListEntry};
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_network::ServerSideConnection;
use std::net::IpAddr;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ServerProfile {
    Local,
    #[default]
    Public,
}

fn is_loopback(address: IpAddr) -> bool {
    address.to_canonical().is_loopback()
}

pub(crate) fn grant_loopback_ops(
    profile: On<Insert<GameProfile>>,
    profile_setting: Res<ServerProfile>,
    connections: Query<(&ServerSideConnection, &GameProfile)>,
    mut ops: ResMut<OpList>,
) {
    if *profile_setting != ServerProfile::Local {
        return;
    }
    let Ok((connection, profile)) = connections.get(profile.event().entity) else {
        return;
    };
    if !is_loopback(connection.remote_addr().ip()) {
        return;
    }
    if ops.grant_loopback(OpListEntry::owner(profile.id, profile.username.clone())) {
        tracing::info!(name = %profile.username, "a loopback join makes the player an operator");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ServerNetworkPlugin;
    use crate::login::LoginPlugin;
    use bevy_app::App;
    use mcrs_minecraft_engine::ability::PlayerOpLevel;
    use mcrs_minecraft_engine_world::session::PlayerSessionCounter;
    use mcrs_minecraft_network::RawConnection;
    use mcrs_minecraft_network::event::Received;
    use mcrs_minecraft_network::{ConnectionState, client::offline_player_uuid};
    use mcrs_minecraft_protocol::packets::login::serverbound::ServerboundHello;
    use mcrs_minecraft_protocol::uuid::Uuid;
    use mcrs_minecraft_protocol::{Bounded, Encode};

    const LISTED: &str = "Listed";

    fn server(profile: Option<ServerProfile>) -> App {
        let mut app = App::new();
        app.add_plugins(LoginPlugin);
        app.init_resource::<PlayerSessionCounter>();
        if let Some(profile) = profile {
            app.insert_resource(profile);
        }
        app.insert_resource(OpList::new([OpListEntry {
            level: 2,
            ..OpListEntry::owner(offline_player_uuid(LISTED), LISTED.to_owned())
        }]));
        app
    }

    fn hello_from(app: &mut App, address: &str, username: &str) -> Option<OpListEntry> {
        let (mut raw, _outgoing, _inbound) = RawConnection::new_for_test_full(16);
        raw.remote_addr = address.parse().unwrap();
        let connection = app
            .world_mut()
            .spawn((
                ServerSideConnection { raw: Box::new(raw) },
                ConnectionState::Login,
            ))
            .id();
        let mut data = Vec::new();
        ServerboundHello {
            username: Bounded(username),
            profile_id: Uuid::nil(),
        }
        .encode(&mut data)
        .unwrap();
        app.world_mut()
            .trigger(Received::<ServerboundHello<'static>>::new(
                connection,
                data.into(),
            ));
        app.world_mut().flush();
        let id = app.world().get::<GameProfile>(connection).unwrap().id;
        app.world().resource::<OpList>().get(&id).cloned()
    }

    #[test]
    fn loop_grant_001_loopback_join_on_local_makes_an_owner() {
        let mut app = server(Some(ServerProfile::Local));
        let op = hello_from(&mut app, "127.0.0.1:50000", "Local").unwrap();
        assert_eq!(op.level, PlayerOpLevel::MAX);
        assert!(!op.bypasses_player_limit);
    }

    #[test]
    fn loop_grant_001_a_loopback_join_of_a_listed_player_keeps_its_entry() {
        let mut app = server(Some(ServerProfile::Local));
        let op = hello_from(&mut app, "127.0.0.1:50000", LISTED).unwrap();
        assert_eq!(op.level, 2);
    }

    #[test]
    fn loop_grant_002_a_loopback_join_on_public_makes_no_operator() {
        let mut app = server(Some(ServerProfile::Public));
        assert_eq!(hello_from(&mut app, "127.0.0.1:50000", "Local"), None);
    }

    #[test]
    fn loop_grant_002_a_listed_player_on_public_has_the_level_of_its_entry() {
        let mut app = server(Some(ServerProfile::Public));
        let op = hello_from(&mut app, "127.0.0.1:50000", LISTED).unwrap();
        assert_eq!(op.level, 2);
    }

    #[test]
    fn loop_addr_001_v6_loopback_and_mapped_v4_loopback_count_on_local() {
        let mut app = server(Some(ServerProfile::Local));
        for (address, name) in [
            ("[::1]:50000", "Six"),
            ("[::ffff:127.0.0.1]:50000", "Mapped"),
        ] {
            let op = hello_from(&mut app, address, name);
            assert_eq!(op.map(|op| op.level), Some(PlayerOpLevel::MAX), "{address}");
        }
    }

    #[test]
    fn loop_grant_001_a_remote_join_on_local_makes_no_operator() {
        let mut app = server(Some(ServerProfile::Local));
        assert_eq!(hello_from(&mut app, "192.168.1.20:50000", "Remote"), None);
    }

    #[test]
    fn loop_cfg_001_the_default_profile_makes_no_operator() {
        assert_eq!(
            ServerNetworkPlugin::default().profile,
            ServerProfile::Public
        );
        assert_eq!(
            ServerNetworkPlugin::embedded().profile,
            ServerProfile::Public
        );
        let mut app = server(None);
        assert_eq!(hello_from(&mut app, "127.0.0.1:50000", "Local"), None);
    }

    #[test]
    fn loopback_covers_the_v4_block_v6_and_mapped_v4_only() {
        for local in ["127.0.0.1", "127.255.0.9", "::1", "::ffff:127.0.0.1"] {
            assert!(is_loopback(local.parse().unwrap()), "{local}");
        }
        for remote in [
            "192.168.1.2",
            "10.0.0.1",
            "::",
            "0.0.0.0",
            "::ffff:10.0.0.1",
            "fe80::1",
        ] {
            assert!(!is_loopback(remote.parse().unwrap()), "{remote}");
        }
    }
}
