use bevy::prelude::*;
use mcrs_minecraft_damage::Vitals;
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
    pub respawn_screen: RespawnScreen,
}

pub(crate) fn on_player_combat_kill(
    event: On<Received<ClientboundPlayerCombatKill<'static>>>,
    connections: Query<&JoinedGame>,
    screen: Query<&RespawnScreen, With<Player>>,
    mut vitals: Query<&mut Vitals, With<Player>>,
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
    if let Ok(mut vitals) = vitals.single_mut()
        && vitals.is_alive()
    {
        vitals.death_time = Some(0);
    }
    let respawn_screen = screen.single().copied().unwrap_or_default();
    let message = chat_packets::server_text(&packet.message).unwrap_or_else(|| Text::text(""));
    killed.write(PlayerKilled {
        message,
        respawn_screen,
    });
    if respawn_screen == RespawnScreen::Immediate {
        respawn.write(RequestRespawn);
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use bevy::math::DVec3;
    use mcrs_minecraft_damage::Vitals;
    use mcrs_minecraft_game_mode::GameMode;
    use mcrs_minecraft_protocol::{Decode, Packet, VarInt};
    use mcrs_minecraft_protocol_game::clientbound::{
        ClientboundPlayerCombatKill, ClientboundSetHealth,
    };
    use mcrs_minecraft_protocol_game::serverbound::{
        ClientCommandAction, ServerboundClientCommand,
    };
    use mcrs_minecraft_protocol_game::text::{RawText, Text};

    use crate::respawn_screen::RespawnScreen;
    use crate::test_session::{Joined, OWN_PLAYER, TestSession, joined, login, receive};

    #[test]
    fn a_kill_of_the_own_player_without_the_screen_requests_a_respawn_at_once() {
        let session = TestSession::new(DVec3::ZERO);
        let player = session.player;
        let Joined {
            mut app,
            mut server,
            connection,
        } = joined(session.app);
        receive(&mut app, connection, login(GameMode::Survival));
        app.world_mut()
            .entity_mut(player)
            .insert(RespawnScreen::Immediate);
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

    #[test]
    fn a_kill_of_the_own_player_marks_it_dead_until_a_living_health_arrives() {
        let mut session = TestSession::new(DVec3::ZERO);
        let alive = |session: &TestSession| {
            session
                .app
                .world()
                .get::<Vitals>(session.player)
                .unwrap()
                .is_alive()
        };
        let kill = |player_id: i32| ClientboundPlayerCombatKill {
            player_id: VarInt(player_id),
            message: RawText::from_text(&Text::text("fell")).unwrap(),
        };

        session.receive(kill(OWN_PLAYER + 1));
        assert!(alive(&session), "the kill of another player");
        session.receive(kill(OWN_PLAYER));
        assert!(!alive(&session), "the kill of the own player");
        session.receive(ClientboundSetHealth {
            health: 20.0,
            food: VarInt(20),
            saturation: 5.0,
        });
        assert!(alive(&session), "a living health after the kill");
    }
}
