use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_mode::PlayerGameMode;

pub(crate) fn ignores_attacks(
    unloaded: bool,
    mode: &PlayerGameMode,
    vitals: Option<&Vitals>,
) -> bool {
    unloaded || mode.0 == GameMode::Spectator || vitals.is_some_and(|vitals| !vitals.is_alive())
}
