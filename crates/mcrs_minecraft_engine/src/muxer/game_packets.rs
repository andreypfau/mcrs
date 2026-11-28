use mcrs_minecraft_network::event::Triggers;
use mcrs_minecraft_network::metrics::PlayTable;
use mcrs_minecraft_protocol_game::table::game_serverbound;

pub const GAME_SERVERBOUND: PlayTable = PlayTable::new(
    &game_serverbound::TABLE,
    game_serverbound::visit::<Triggers>,
);
