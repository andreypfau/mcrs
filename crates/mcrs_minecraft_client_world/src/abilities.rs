use bevy::prelude::*;
use mcrs_minecraft_game_mode::{Abilities, GameMode};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundGameEvent, ClientboundLogin, ClientboundPlayerAbilities, ClientboundRespawn,
};
use mcrs_minecraft_protocol_game::game_event::GameEventKind;
use mcrs_minecraft_protocol_game::player::PlayerSpawnInfo;

use crate::local_motion::LocalMotion;
use crate::player::Player;

#[must_use]
pub(crate) fn received(abilities: Abilities, packet: ClientboundPlayerAbilities) -> Abilities {
    Abilities {
        invulnerable: packet.invulnerable,
        may_fly: packet.may_fly,
        instant_build: packet.instant_build,
        flying_speed: packet.flying_speed,
        walking_speed: packet.walking_speed,
        ..abilities
    }
}

#[must_use]
pub(crate) fn flight_after_change(mode: GameMode, flying: bool) -> bool {
    match mode {
        GameMode::Survival | GameMode::Adventure => false,
        GameMode::Creative => flying,
        GameMode::Spectator => true,
    }
}

pub(crate) fn on_login(
    event: On<Received<ClientboundLogin>>,
    player: Query<Entity, With<Player>>,
    commands: Commands,
) {
    if let Some(login) = event.packet() {
        spawned(&login.player_spawn_info, player, commands);
    }
}

pub(crate) fn on_respawn(
    event: On<Received<ClientboundRespawn>>,
    player: Query<Entity, With<Player>>,
    commands: Commands,
) {
    if let Some(respawn) = event.packet() {
        spawned(&respawn.player_spawn_info, player, commands);
    }
}

fn spawned(info: &PlayerSpawnInfo, player: Query<Entity, With<Player>>, mut commands: Commands) {
    if let Ok(player) = player.single() {
        commands
            .entity(player)
            .insert(Abilities::new(info.game_mode));
    }
}

pub(crate) fn on_game_event(
    event: On<Received<ClientboundGameEvent>>,
    mut abilities: Query<&mut Abilities, With<Player>>,
    mut motion: Query<&mut LocalMotion, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let GameEventKind::ChangeGameMode(mode) = packet.game_event else {
        return;
    };
    if let Ok(mut abilities) = abilities.single_mut() {
        let next = abilities.changed_to(mode);
        abilities.set_if_neq(next);
    }
    if let Ok(motion) = motion.single_mut() {
        let flying = flight_after_change(mode, motion.0.flight0);
        motion
            .map_unchanged(|motion| &mut motion.0.flight0)
            .set_if_neq(flying);
    }
}

pub(crate) fn on_player_abilities(
    event: On<Received<ClientboundPlayerAbilities>>,
    mut abilities: Query<&mut Abilities, With<Player>>,
    mut motion: Query<&mut LocalMotion, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if let Ok(mut abilities) = abilities.single_mut() {
        let next = received(*abilities, packet);
        abilities.set_if_neq(next);
    }
    if let Ok(motion) = motion.single_mut() {
        motion
            .map_unchanged(|motion| &mut motion.0.flight0)
            .set_if_neq(packet.flying);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_session::{FLOOR_TOP, TestSession, login, spawn_info};
    use bevy::math::DVec3;

    fn change(mode: GameMode) -> ClientboundGameEvent {
        ClientboundGameEvent {
            game_event: GameEventKind::ChangeGameMode(mode),
        }
    }

    enum Step {
        Login(GameMode),
        Respawn(GameMode),
        Change(GameMode),
        Packet(ClientboundPlayerAbilities),
    }

    const SURVIVAL: Abilities = Abilities {
        invulnerable: false,
        may_fly: false,
        instant_build: false,
        may_build: true,
        flying_speed: 0.05_f32,
        walking_speed: 0.1_f32,
    };
    const CREATIVE: Abilities = Abilities {
        invulnerable: true,
        may_fly: true,
        instant_build: true,
        ..SURVIVAL
    };
    const ADVENTURE: Abilities = Abilities {
        may_build: false,
        ..SURVIVAL
    };
    const SPECTATOR: Abilities = Abilities {
        invulnerable: true,
        may_fly: true,
        may_build: false,
        ..SURVIVAL
    };

    struct Row {
        step: Step,
        abilities: Abilities,
        flight0: bool,
    }

    #[test]
    fn login_and_mode_changes_set_abilities_and_flight_state() {
        let flying_creative = ClientboundPlayerAbilities {
            invulnerable: true,
            flying: true,
            may_fly: true,
            instant_build: true,
            flying_speed: 0.1_f32,
            walking_speed: 0.2_f32,
        };
        let rows = [
            Row {
                step: Step::Login(GameMode::Survival),
                abilities: SURVIVAL,
                flight0: false,
            },
            Row {
                step: Step::Login(GameMode::Creative),
                abilities: CREATIVE,
                flight0: false,
            },
            Row {
                step: Step::Change(GameMode::Survival),
                abilities: SURVIVAL,
                flight0: false,
            },
            Row {
                step: Step::Change(GameMode::Creative),
                abilities: CREATIVE,
                flight0: false,
            },
            Row {
                step: Step::Change(GameMode::Spectator),
                abilities: SPECTATOR,
                flight0: true,
            },
            Row {
                step: Step::Change(GameMode::Creative),
                abilities: CREATIVE,
                flight0: true,
            },
            Row {
                step: Step::Change(GameMode::Survival),
                abilities: SURVIVAL,
                flight0: false,
            },
            Row {
                step: Step::Change(GameMode::Adventure),
                abilities: ADVENTURE,
                flight0: false,
            },
            Row {
                step: Step::Packet(flying_creative),
                abilities: Abilities {
                    may_build: false,
                    flying_speed: 0.1_f32,
                    walking_speed: 0.2_f32,
                    ..CREATIVE
                },
                flight0: true,
            },
            Row {
                step: Step::Change(GameMode::Creative),
                abilities: Abilities {
                    flying_speed: 0.1_f32,
                    walking_speed: 0.2_f32,
                    ..CREATIVE
                },
                flight0: true,
            },
            Row {
                step: Step::Respawn(GameMode::Creative),
                abilities: CREATIVE,
                flight0: false,
            },
            Row {
                step: Step::Respawn(GameMode::Spectator),
                abilities: SPECTATOR,
                flight0: true,
            },
        ];

        let mut session = TestSession::new(DVec3::new(8.5, FLOOR_TOP, 8.5));
        session.receive(change(GameMode::Creative));
        assert_eq!(session.app.world().get::<Abilities>(session.player), None);

        for (index, row) in rows.into_iter().enumerate() {
            match row.step {
                Step::Login(mode) => session.receive(login(mode)),
                Step::Respawn(mode) => session.receive(ClientboundRespawn {
                    player_spawn_info: spawn_info(mode),
                    data_to_keep: 0,
                }),
                Step::Change(mode) => session.receive(change(mode)),
                Step::Packet(packet) => session.receive(packet),
            }
            assert_eq!(
                session.app.world().get::<Abilities>(session.player),
                Some(&row.abilities),
                "step {index}"
            );
            assert_eq!(session.motion().flight0, row.flight0, "step {index}");
        }
    }
}
