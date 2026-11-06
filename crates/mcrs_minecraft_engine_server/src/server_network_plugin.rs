use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use bevy_app::{App, Plugin};
use mcrs_minecraft_engine::client_info::ClientInfoPlugin;
use mcrs_minecraft_engine::whitelist::Whitelist;
use mcrs_minecraft_network::{NetworkPlugin, default_favicon};

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
    pub favicon: Option<String>,
    /// Turned on, a client that a transfer packet sends here logs in; turned off, it gets
    /// `multiplayer.disconnect.transfers_disabled`.
    pub accepts_transfers: bool,
    /// Turned on, the status response keeps the online and maximum counts but carries no sample
    /// of player names.
    pub hide_online_players: bool,
    /// Turned off, a status connection closes after the handshake without a packet; a login is
    /// not affected.
    pub enable_status: bool,
    /// The kernel send buffer of each connection, in bytes.
    pub send_buffer_bytes: usize,
    /// The length of the kernel queue of connections that wait for the accept loop.
    pub accept_backlog: u32,
    pub whitelist: Whitelist,
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
            favicon: Some(default_favicon()),
            accepts_transfers: false,
            hide_online_players: false,
            enable_status: true,
            send_buffer_bytes: NetworkPlugin::default().send_buffer_bytes,
            accept_backlog: NetworkPlugin::default().accept_backlog,
            whitelist: Whitelist::default(),
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
            favicon: self.favicon.clone(),
            accepts_transfers: self.accepts_transfers,
            hide_online_players: self.hide_online_players,
            enable_status: self.enable_status,
            send_buffer_bytes: self.send_buffer_bytes,
            accept_backlog: self.accept_backlog,
        });
        app.insert_resource(self.compression_threshold);
        app.add_plugins(LoginPlugin);
        app.add_plugins(ConfigurationStatePlugin);
        app.add_plugins(KeepAlivePlugin);
        if !app.is_plugin_added::<ClientInfoPlugin>() {
            app.add_plugins(ClientInfoPlugin);
        }
        app.add_plugins(StatusPlugin);
        app.add_plugins(crate::shutdown::ShutdownPlugin);
        app.insert_resource(self.whitelist.clone());
    }
}
