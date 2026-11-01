use mcrs_minecraft_network::event::Triggers;
use mcrs_minecraft_network::metrics::PlayTable;
use mcrs_minecraft_protocol_game::table::game_serverbound;

/// The serverbound play rows with game data, which the bridge dispatches.
pub const GAME_SERVERBOUND: PlayTable = PlayTable::new(
    &game_serverbound::TABLE,
    game_serverbound::visit::<Triggers>,
);
