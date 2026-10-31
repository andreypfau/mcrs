use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use bevy_app::{App, Plugin};
use mcrs_minecraft_engine::client_info::ClientInfoPlugin;
use mcrs_minecraft_network::NetworkPlugin;

use crate::configuration::{ConfigurationStatePlugin, KnownPackOffer};
use crate::dimension_worlds_plugin::DimensionWorldsPlugin;
use crate::keep_alive::KeepAlivePlugin;
use crate::login::{LoginPlugin, NetworkCompressionThreshold, SingleplayerProfile};
use crate::require::require;
use crate::status::StatusPlugin;

/// The listener, login, configuration and keep-alive of every connection.
pub struct ServerNetworkPlugin {
    /// Port 0 asks the OS for a free port; read the result back from
    /// [`BoundAddress`](mcrs_minecraft_network::BoundAddress).
    pub bind_address: SocketAddr,
    /// A server that listens beyond loopback announces itself on the local network.
    pub announce_on_lan: bool,
    /// Turned off, the server offers no known pack, so a client receives every registry entry's
    /// data instead of only the entries that are not in the vanilla pack.
    pub offer_known_packs: bool,
    pub compression_threshold: NetworkCompressionThreshold,
    pub singleplayer_profile: Option<SingleplayerProfile>,
    pub max_players: u32,
    pub motd: String,
    /// Turned on, a client that a transfer packet sends here logs in; turned off, it gets
    /// `multiplayer.disconnect.transfers_disabled`.
    pub accepts_transfers: bool,
    pub hide_online_players: bool,
    pub enable_status: bool,
}

impl ServerNetworkPlugin {
    /// A listener on loopback only, on an OS-assigned port and without compression, for a
    /// server that shares a process with its client.
    pub fn embedded() -> Self {
        Self {
            bind_address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(),
            // Both ends share one process, so compression only spends CPU on each side.
            compression_threshold: NetworkCompressionThreshold(-1),
            ..Default::default()
        }
    }
}

impl Default for ServerNetworkPlugin {
    fn default() -> Self {
        Self {
            bind_address: SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 25565).into(),
            announce_on_lan: true,
            offer_known_packs: true,
            compression_threshold: NetworkCompressionThreshold::default(),
            singleplayer_profile: None,
            max_players: 20,
            motd: "mcrs Server".to_owned(),
            accepts_transfers: false,
            hide_online_players: false,
            enable_status: true,
        }
    }
}

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        require::<Self, DimensionWorldsPlugin>(app);
        app.insert_resource(KnownPackOffer(self.offer_known_packs));
        if let Some(host) = &self.singleplayer_profile {
            app.insert_resource(host.clone());
        }
        app.add_plugins(NetworkPlugin {
            address: self.bind_address,
            announce_on_lan: self.announce_on_lan,
            max_players: self.max_players,
            motd: self.motd.clone(),
            accepts_transfers: self.accepts_transfers,
            hide_online_players: self.hide_online_players,
            enable_status: self.enable_status,
        });
        app.insert_resource(self.compression_threshold);
        app.add_plugins(LoginPlugin);
        app.add_plugins(ConfigurationStatePlugin);
        app.add_plugins(KeepAlivePlugin);
        if !app.is_plugin_added::<ClientInfoPlugin>() {
            app.add_plugins(ClientInfoPlugin);
        }
        app.add_plugins(StatusPlugin);
    }
}
