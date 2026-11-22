use bevy::prelude::*;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::ClientboundPlayerCombatKill;
use mcrs_minecraft_protocol_game::text::Text;

use crate::chat_packets;
use crate::joined_game::JoinedGame;
use crate::player::Player;
use crate::request_respawn::RequestRespawn;
use crate::respawn_screen::RespawnScreen;

#[derive(Message, Clone, Debug)]
pub struct PlayerKilled {
    pub message: Text,
}

pub(crate) fn on_player_combat_kill(
    event: On<Received<ClientboundPlayerCombatKill<'static>>>,
    connections: Query<&JoinedGame>,
    screen: Query<&RespawnScreen, With<Player>>,
    mut killed: MessageWriter<PlayerKilled>,
    mut respawn: MessageWriter<RequestRespawn>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if !connections
        .get(event.entity)
        .is_ok_and(|joined| joined.player_id == packet.player_id.0)
    {
        return;
    }
    let message = chat_packets::server_text(&packet.message).unwrap_or_else(|| Text::text(""));
    killed.write(PlayerKilled { message });
    if screen
        .single()
        .is_ok_and(|screen| *screen == RespawnScreen::Immediate)
    {
        respawn.write(RequestRespawn);
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use mcrs_minecraft_game_mode::GameMode;
    use mcrs_minecraft_network::client::{
        ClientConnection, ClientNetworkPlugin, JoinRequest, finish_configuration,
    };
    use mcrs_minecraft_protocol::{Decode, Packet, VarInt};
    use mcrs_minecraft_protocol_game::clientbound::{
        ClientboundLogin, ClientboundPlayerCombatKill,
    };
    use mcrs_minecraft_protocol_game::serverbound::{
        ClientCommandAction, ServerboundClientCommand,
    };
    use mcrs_minecraft_protocol_game::text::{RawText, Text};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    use crate::joined_game::JoinedGame;
    use crate::test_session::{OWN_PLAYER, Server, TestSession, login, receive};

    #[test]
    fn a_kill_of_the_own_player_without_the_screen_requests_a_respawn_at_once() {
        let mut app = TestSession::new(DVec3::ZERO).app;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        app.add_plugins(ClientNetworkPlugin {
            holds_vanilla_pack: false,
            play: None,
        });
        app.world_mut().write_message(JoinRequest {
            server: listener.local_addr().unwrap(),
            username: "Steve".to_owned(),
            profile_id: None,
            view_distance: 8,
        });
        app.update();
        let (mut server, _information) = Server::join(&listener);
        let connection = (0..1000)
            .find_map(|_| {
                app.update();
                thread::sleep(Duration::from_millis(10));
                app.world_mut()
                    .query_filtered::<Entity, With<ClientConnection>>()
                    .single(app.world())
                    .ok()
            })
            .expect("the client joins");
        finish_configuration(&mut app.world_mut().commands(), connection);
        app.world_mut().flush();
        app.update();
        let _finished = server.next_frame();
        app.world_mut().entity_mut(connection).insert(JoinedGame {
            player_id: OWN_PLAYER,
            dimensions: Vec::new(),
        });
        receive(
            &mut app,
            connection,
            ClientboundLogin {
                show_death_screen: false,
                ..login(GameMode::Survival)
            },
        );
        receive(
            &mut app,
            connection,
            ClientboundPlayerCombatKill {
                player_id: VarInt(OWN_PLAYER),
                message: RawText::from_text(&Text::text("fell")).unwrap(),
            },
        );
        app.update();
        let request = loop {
            let frame = server.next_frame();
            if frame.id == ServerboundClientCommand::ID {
                break ServerboundClientCommand::decode(&mut &frame.body[..]).unwrap();
            }
        };
        assert_eq!(request.action, ClientCommandAction::PerformRespawn);
    }
}
