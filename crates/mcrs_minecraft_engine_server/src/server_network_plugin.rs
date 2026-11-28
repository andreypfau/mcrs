use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

use bevy_app::{App, Plugin};
use mcrs_minecraft_engine::banlist::BanList;
use mcrs_minecraft_engine::client_info::ClientInfoPlugin;
use mcrs_minecraft_engine::whitelist::Whitelist;
use mcrs_minecraft_nbt::NbtBudget;
use mcrs_minecraft_network::inbound_rate::InboundRateLimit;
use mcrs_minecraft_network::{ForwardingSecret, NetworkPlugin, default_favicon};

use crate::configuration::{ConfigurationStatePlugin, KnownPackOffer};
use crate::dimension_worlds_plugin::DimensionWorldsPlugin;
use crate::keep_alive::KeepAlivePlugin;
use crate::login::{
    JoinLimits, LoginPlugin, NetworkCompressionThreshold, ProxyForwarding, SingleplayerProfile,
};
use crate::loopback_ops::ServerProfile;
use crate::require::require;
use crate::status::StatusPlugin;

pub struct ServerNetworkPlugin {
    pub bind_address: SocketAddr,
    pub announce_on_lan: bool,
    pub offer_known_packs: bool,
    pub compression_threshold: NetworkCompressionThreshold,
    pub singleplayer_profile: Option<SingleplayerProfile>,
    pub join_limits: JoinLimits,
    pub motd: String,
    pub favicon: Option<String>,
    pub accepts_transfers: bool,
    pub hide_online_players: bool,
    pub enable_status: bool,
    pub send_buffer_bytes: usize,
    pub accept_backlog: u32,
    pub rate_limit: InboundRateLimit,
    pub nbt_budget: NbtBudget,
    pub whitelist: Whitelist,
    pub banlist: BanList,
    pub profile: ServerProfile,
    pub forwarding: Option<ForwardingSecret>,
    pub proxy_upstreams: Vec<IpAddr>,
}

impl ServerNetworkPlugin {
    pub fn embedded() -> Self {
        Self {
            bind_address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(),
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
            join_limits: JoinLimits::default(),
            motd: "mcrs Server".to_owned(),
            favicon: Some(default_favicon()),
            accepts_transfers: false,
            hide_online_players: false,
            enable_status: true,
            send_buffer_bytes: NetworkPlugin::default().send_buffer_bytes,
            accept_backlog: NetworkPlugin::default().accept_backlog,
            rate_limit: InboundRateLimit::default(),
            nbt_budget: NbtBudget::default(),
            whitelist: Whitelist::default(),
            banlist: BanList::default(),
            profile: ServerProfile::default(),
            forwarding: None,
            proxy_upstreams: Vec::new(),
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
            max_players: self.join_limits.max_players,
            motd: self.motd.clone(),
            favicon: self.favicon.clone(),
            accepts_transfers: self.accepts_transfers,
            hide_online_players: self.hide_online_players,
            enable_status: self.enable_status,
            send_buffer_bytes: self.send_buffer_bytes,
            accept_backlog: self.accept_backlog,
            rate_limit: self.rate_limit,
            nbt_budget: self.nbt_budget,
            per_ip_accept_limit: self.forwarding.is_none(),
            proxy_upstreams: self.proxy_upstreams.clone(),
        });
        app.insert_resource(self.compression_threshold);
        app.add_plugins(LoginPlugin);
        app.insert_resource(self.join_limits);
        app.add_plugins(ConfigurationStatePlugin);
        app.add_plugins(KeepAlivePlugin);
        if !app.is_plugin_added::<ClientInfoPlugin>() {
            app.add_plugins(ClientInfoPlugin);
        }
        app.add_plugins(StatusPlugin);
        app.add_plugins(crate::shutdown::ShutdownPlugin);
        app.insert_resource(self.whitelist.clone());
        app.insert_resource(self.banlist.clone());
        app.insert_resource(self.profile);
        if let Some(secret) = &self.forwarding {
            app.insert_resource(ProxyForwarding(secret.clone()));
        }
    }
}
