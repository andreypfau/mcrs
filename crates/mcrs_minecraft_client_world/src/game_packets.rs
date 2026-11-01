use mcrs_minecraft_network::event::Triggers;
use mcrs_minecraft_network::metrics::PlayTable;
use mcrs_minecraft_protocol_game::table::game_clientbound;

/// The clientbound play rows with game data, for `ClientNetworkPlugin::play`.
pub const GAME_CLIENTBOUND: PlayTable = PlayTable::new(
    &game_clientbound::TABLE,
    game_clientbound::visit::<Triggers>,
);
