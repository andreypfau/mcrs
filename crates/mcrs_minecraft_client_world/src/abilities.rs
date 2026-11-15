use bevy::prelude::*;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundGameEvent, ClientboundLogin, ClientboundPlayerAbilities, ClientboundRespawn,
};
use mcrs_minecraft_protocol_game::game_event::GameEventKind;
use mcrs_minecraft_protocol_game::player::PlayerSpawnInfo;

use crate::player::Player;

/// The abilities of the local [`Player`]: the table of its game mode (CMD-GM-007) and the values
/// of the last clientbound `player_abilities` (CMD-GM-008). Absent until the login packet.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Abilities {
    pub invulnerable: bool,
    pub flying: bool,
    pub may_fly: bool,
    pub instant_build: bool,
    /// Not on the wire: only the game mode sets it.
    pub may_build: bool,
    /// The flight speed of the kernel (PHY-INP-013), `0.05` by default (CMD-GM-007).
    pub flying_speed: f32,
    /// The walking speed of the zoom of the camera (CCAM-FOV-002), `0.1` by default (CMD-GM-007).
    pub walking_speed: f32,
}

impl Abilities {
    /// The abilities of a new local player in `mode`, which does not fly (PHY-ORD-015).
    #[must_use]
    pub fn new(mode: GameMode) -> Self {
        Self {
            invulnerable: false,
            flying: false,
            may_fly: false,
            instant_build: false,
            may_build: false,
            flying_speed: 0.05_f32,
            walking_speed: 0.1_f32,
        }
        .changed_to(mode)
    }

    /// The abilities after a change to `mode`: creative keeps the flight state, and no mode
    /// changes the speeds (CMD-GM-007).
    #[must_use]
    pub fn changed_to(self, mode: GameMode) -> Self {
        let (invulnerable, flying, may_fly, instant_build, may_build) = match mode {
            GameMode::Survival => (false, false, false, false, true),
            GameMode::Creative => (true, self.flying, true, true, true),
            GameMode::Adventure => (false, false, false, false, false),
            GameMode::Spectator => (true, true, true, false, false),
        };
        Self {
            invulnerable,
            flying,
            may_fly,
            instant_build,
            may_build,
            ..self
        }
    }

    /// Takes every value of `packet` and keeps `may_build` (MVC-PKT-024).
    #[must_use]
    pub fn received(self, packet: ClientboundPlayerAbilities) -> Self {
        Self {
            invulnerable: packet.invulnerable,
            flying: packet.flying,
            may_fly: packet.may_fly,
            instant_build: packet.instant_build,
            flying_speed: packet.flying_speed,
            walking_speed: packet.walking_speed,
            ..self
        }
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
) {
    if let Some(packet) = event.packet()
        && let GameEventKind::ChangeGameMode(mode) = packet.game_event
        && let Ok(mut abilities) = abilities.single_mut()
    {
        let next = abilities.changed_to(mode);
        abilities.set_if_neq(next);
    }
}

pub(crate) fn on_player_abilities(
    event: On<Received<ClientboundPlayerAbilities>>,
    mut abilities: Query<&mut Abilities, With<Player>>,
) {
    if let Some(packet) = event.packet()
        && let Ok(mut abilities) = abilities.single_mut()
    {
        let next = abilities.received(packet);
        abilities.set_if_neq(next);
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
        flying: false,
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
        flying: true,
        may_fly: true,
        may_build: false,
        ..SURVIVAL
    };

    struct Row {
        step: Step,
        abilities: Abilities,
    }

    #[test]
    fn login_game_mode_changes_and_player_abilities_set_the_abilities() {
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
            },
            Row {
                step: Step::Login(GameMode::Creative),
                abilities: CREATIVE,
            },
            Row {
                step: Step::Change(GameMode::Survival),
                abilities: SURVIVAL,
            },
            Row {
                step: Step::Change(GameMode::Creative),
                abilities: CREATIVE,
            },
            Row {
                step: Step::Change(GameMode::Spectator),
                abilities: SPECTATOR,
            },
            Row {
                step: Step::Change(GameMode::Creative),
                abilities: Abilities {
                    flying: true,
                    ..CREATIVE
                },
            },
            Row {
                step: Step::Change(GameMode::Survival),
                abilities: SURVIVAL,
            },
            Row {
                step: Step::Change(GameMode::Adventure),
                abilities: ADVENTURE,
            },
            Row {
                step: Step::Packet(flying_creative),
                abilities: Abilities {
                    flying: true,
                    may_build: false,
                    flying_speed: 0.1_f32,
                    walking_speed: 0.2_f32,
                    ..CREATIVE
                },
            },
            Row {
                step: Step::Change(GameMode::Creative),
                abilities: Abilities {
                    flying: true,
                    flying_speed: 0.1_f32,
                    walking_speed: 0.2_f32,
                    ..CREATIVE
                },
            },
            Row {
                step: Step::Respawn(GameMode::Creative),
                abilities: CREATIVE,
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
        }
    }
}
