use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::{Add, Insert};
use bevy_ecs::prelude::{On, Query};
use bevy_ecs::query::{Has, With, Without};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::any_with_component;
use bevy_ecs::system::{Commands, Res, ResMut, SystemParam};
use mcrs_minecraft_network::client::offline_player_uuid;
use mcrs_minecraft_network::connect::TokenBucket;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_network::{
    ConnectionState, ForwardingRefusal, ForwardingSecret, ServerSideConnection, forwarding_query,
};
use mcrs_minecraft_protocol::packets::login::serverbound::{
    ServerboundCustomQueryAnswer, ServerboundHello, ServerboundLoginAcknowledged,
};
use mcrs_minecraft_protocol::{CompressionThreshold, WritePacket, uuid};
use mcrs_minecraft_text::Text;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use crate::loopback_ops::{ServerProfile, grant_loopback_ops};
use crate::session_state::{
    LoginState, SessionsById, client_protocol, disconnect, duplicate_login_reason,
};

use mcrs_minecraft_engine::banlist::{BanEntry, BanList, EnforceBans};
use mcrs_minecraft_engine::coordinator::ops::OpList;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::muxer::disconnect::Departing;
use mcrs_minecraft_engine::muxer::session::{HostAnchorRef, SessionBundle};
use mcrs_minecraft_engine::whitelist::{EnforceWhitelist, Whitelist};
use mcrs_minecraft_engine_world::session::{Place, PlayerSessionCounter};

#[derive(Resource, Clone, Copy, Debug)]
pub struct ChatSessionId(pub uuid::Uuid);

#[derive(Resource, Clone, Copy, Debug)]
pub struct NetworkCompressionThreshold(pub i32);

impl Default for NetworkCompressionThreshold {
    fn default() -> Self {
        Self(256)
    }
}

pub struct LoginPlugin;

impl bevy_app::Plugin for LoginPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.insert_resource(ChatSessionId(uuid::Uuid::new_v4()));
        app.init_resource::<ServerTicks>();
        app.init_resource::<Whitelist>();
        app.init_resource::<BanList>();
        app.init_resource::<OpList>();
        app.init_resource::<ServerProfile>();
        app.init_resource::<NetworkCompressionThreshold>();
        app.init_resource::<JoinLimits>();
        app.init_resource::<JoinBuckets>();
        app.add_systems(
            bevy_app::First,
            advance_server_ticks.after(bevy_ecs::message::MessageUpdateSystems),
        );
        app.add_observer(start_login_clock);
        app.add_observer(handle_hello_packet);
        app.add_observer(handle_custom_query_answer);
        app.add_observer(handle_login_acknowledged);
        app.add_observer(on_login_accepted);
        app.add_observer(grant_loopback_ops);
        app.add_systems(
            bevy_app::Update,
            (end_slow_logins, finish_awaiting_logins)
                .chain()
                .after(mcrs_minecraft_network::event::run_event_loop)
                .before(crate::configuration::request_initial_player_spawn)
                .before(crate::shutdown::shutdown_system)
                .run_if(any_with_component::<LoginStarted>),
        );
        app.add_observer(enforce_bans);
        app.add_observer(enforce_whitelist);
    }
}

fn enforce_whitelist(
    _: On<EnforceWhitelist>,
    whitelist: Res<Whitelist>,
    mut connections: Query<(
        Entity,
        &mut ServerSideConnection,
        &ConnectionState,
        &GameProfile,
        Option<&LoginState>,
    )>,
    mut commands: Commands,
) {
    for (connection, mut con, &state, profile, login) in &mut connections {
        if whitelist.contains(&profile.id) {
            continue;
        }
        tracing::info!(name = %profile.username, "disconnected: not whitelisted");
        disconnect(
            &mut con,
            client_protocol(state, login),
            not_whitelisted_reason(),
        );
        commands.entity(connection).remove::<ServerSideConnection>();
    }
}

fn enforce_bans(
    _: On<EnforceBans>,
    bans: Res<BanList>,
    mut connections: Query<(
        Entity,
        &mut ServerSideConnection,
        &ConnectionState,
        &GameProfile,
        Option<&LoginState>,
    )>,
    mut commands: Commands,
) {
    for (connection, mut con, &state, profile, login) in &mut connections {
        if bans.active(&profile.id).is_none() {
            continue;
        }
        tracing::info!(name = %profile.username, "disconnected: banned");
        disconnect(
            &mut con,
            client_protocol(state, login),
            Text::translate("multiplayer.disconnect.banned", Vec::new()),
        );
        commands.entity(connection).remove::<ServerSideConnection>();
    }
}

const MAX_TICKS_BEFORE_LOGIN: u64 = 600;

#[derive(Resource, Default, Debug, Clone, Copy)]
struct ServerTicks(u64);

#[derive(Component, Debug, Clone, Copy)]
struct LoginStarted(u64);

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "counts every server tick, and the login timeout measures its age against this count"
    )
)]
fn advance_server_ticks(mut ticks: ResMut<ServerTicks>) {
    ticks.0 += 1;
}

fn start_login_clock(
    trigger: On<Add<ConnectionState>>,
    states: Query<&ConnectionState>,
    ticks: Res<ServerTicks>,
    mut commands: Commands,
) {
    let connection = trigger.event().entity;
    if states.get(connection) == Ok(&ConnectionState::Login) {
        commands.entity(connection).insert(LoginStarted(ticks.0));
    }
}

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct SingleplayerProfile {
    pub name: String,
    pub id: uuid::Uuid,
}

fn hello_profile(username: &str, host: Option<&SingleplayerProfile>) -> GameProfile {
    let (id, username) = match host {
        Some(host) if host.name.eq_ignore_ascii_case(username) => (host.id, host.name.clone()),
        _ => (offline_player_uuid(username), username.to_owned()),
    };
    GameProfile {
        id,
        username,
        properties: Vec::new(),
    }
}

fn is_valid_player_name(name: &str) -> bool {
    name.len() <= 16 && name.bytes().all(|byte| (b'!'..=b'~').contains(&byte))
}

fn invalid_player_name_reason() -> Text {
    Text::translate(
        "disconnect.genericReason",
        [Text::text(
            "Internal Exception: java.lang.IllegalStateException: Invalid characters in username",
        )],
    )
}

pub(crate) fn banned_reason(entry: &BanEntry) -> Text {
    let reason = match &entry.reason {
        Some(reason) => Text::text(reason.clone()),
        None => Text::translate("multiplayer.disconnect.banned.reason.default", Vec::new()),
    };
    let mut text = Text::translate("multiplayer.disconnect.banned.reason", vec![reason]);
    if let Some(expiration) = entry.expiration_text() {
        text.extra.push(Text::translate(
            "multiplayer.disconnect.banned.expiration",
            vec![Text::text(expiration)],
        ));
    }
    text
}

pub(crate) fn not_whitelisted_reason() -> Text {
    Text::translate("multiplayer.disconnect.not_whitelisted", Vec::new())
}

fn server_full_reason() -> Text {
    Text::translate("multiplayer.disconnect.server_full", Vec::new())
}

fn too_many_connections_reason() -> Text {
    Text::text("Too many connections from your address")
}

fn too_many_logins_reason() -> Text {
    Text::text("Too many logins from your address. Wait a few seconds and try again.")
}

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct JoinLimits {
    pub max_players: u32,
    pub per_ip: Option<u32>,
    pub join_rate: Option<JoinRate>,
}

impl Default for JoinLimits {
    fn default() -> Self {
        Self {
            max_players: 20,
            per_ip: Some(5),
            join_rate: Some(JoinRate {
                capacity: 3,
                period: Duration::from_secs(4),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JoinRate {
    pub capacity: u32,
    pub period: Duration,
}

#[derive(Resource, Default)]
struct JoinBuckets(HashMap<IpAddr, TokenBucket>);

#[derive(SystemParam)]
pub struct JoinCheck<'w> {
    bans: Res<'w, BanList>,
    whitelist: Res<'w, Whitelist>,
    ops: Res<'w, OpList>,
    host: Option<Res<'w, SingleplayerProfile>>,
}

impl JoinCheck<'_> {
    pub fn allows(&self, id: &uuid::Uuid) -> bool {
        self.whitelist.allows(id) || self.ops.get(id).is_some()
    }
}

#[derive(SystemParam)]
pub struct LoginReplies<'w> {
    session_id: Res<'w, ChatSessionId>,
    compression: Res<'w, NetworkCompressionThreshold>,
}

impl LoginReplies<'_> {
    fn announce_compression(&self, con: &mut ServerSideConnection) {
        if self.compression.0 >= 0 {
            con.raw
                .announce_compression(CompressionThreshold(self.compression.0));
        }
    }

    fn finish(&self, con: &mut ServerSideConnection, profile: &GameProfile) {
        con.raw.finish_login(profile.into(), self.session_id.0);
    }
}

#[derive(Resource, Clone, Debug)]
pub struct ProxyForwarding(pub ForwardingSecret);

#[derive(Component, Debug)]
pub struct AwaitingForwarding;

#[derive(SystemParam)]
pub struct Admission<'w, 's> {
    session_connections: Query<
        'w,
        's,
        (
            &'static mut ServerSideConnection,
            &'static ConnectionState,
            &'static LoginState,
            &'static GameProfile,
        ),
    >,
    sessions: SessionsById<'w, 's>,
    replies: LoginReplies<'w>,
    join_check: JoinCheck<'w>,
    limits: Res<'w, JoinLimits>,
    join_buckets: ResMut<'w, JoinBuckets>,
    commands: Commands<'w, 's>,
}

impl Admission<'_, '_> {
    fn refuse(&mut self, connection: Entity, con: &mut ServerSideConnection, reason: Text) {
        disconnect(con, ConnectionState::Login, reason);
        self.commands
            .entity(connection)
            .remove::<ServerSideConnection>();
    }

    fn limit_refusal(&mut self, ip: IpAddr, id: uuid::Uuid) -> Option<Text> {
        let ip = ip.to_canonical();
        let joined = || {
            self.session_connections
                .iter()
                .filter(|(.., profile)| profile.id != id)
        };
        if joined().count() >= self.limits.max_players as usize {
            return Some(server_full_reason());
        }
        if ip.is_loopback() {
            return None;
        }
        if let Some(cap) = self.limits.per_ip {
            let from_ip = joined()
                .filter(|(con, ..)| con.remote_addr().ip().to_canonical() == ip)
                .count();
            if from_ip >= cap as usize {
                return Some(too_many_connections_reason());
            }
        }
        if let Some(rate) = self.limits.join_rate {
            let refill = 1.0 / rate.period.as_secs_f32();
            self.join_buckets
                .0
                .retain(|_, bucket| !bucket.is_full(rate.capacity, refill));
            let bucket = self
                .join_buckets
                .0
                .entry(ip)
                .or_insert_with(|| TokenBucket::new(rate.capacity));
            if !bucket.consume(rate.capacity, refill) {
                return Some(too_many_logins_reason());
            }
        }
        None
    }

    fn admit(&mut self, connection: Entity, con: &mut ServerSideConnection, profile: GameProfile) {
        if let Some(ban) = self.join_check.bans.active(&profile.id) {
            tracing::info!(name = %profile.username, "login refused: banned");
            let reason = banned_reason(ban);
            self.refuse(connection, con, reason);
            return;
        }
        if !self.join_check.allows(&profile.id) {
            tracing::info!(name = %profile.username, "login refused: not whitelisted");
            self.refuse(connection, con, not_whitelisted_reason());
            return;
        }
        if let Some(reason) = self.limit_refusal(con.remote_addr().ip(), profile.id) {
            tracing::info!(name = %profile.username, "login refused: {reason:?}");
            self.refuse(connection, con, reason);
            return;
        }
        tracing::debug!(?profile, "login hello");
        self.replies.announce_compression(con);
        let mut must_wait = self.sessions.saving(profile.id);
        for (_, place, other) in self.sessions.of(profile.id) {
            let other = other.and_then(|entity| {
                let (con, &state, ..) = self.session_connections.get_mut(entity).ok()?;
                Some((entity, con, state))
            });
            let in_play = matches!(other, Some((_, _, ConnectionState::Game)));
            if place == Place::Unplaced && !in_play {
                continue;
            }
            must_wait = true;
            if let Some((other_connection, mut other, state)) = other {
                tracing::info!(name = %profile.username, "a new login under the same id ends the session");
                disconnect(&mut other, state, duplicate_login_reason());
                self.commands
                    .entity(other_connection)
                    .remove::<ServerSideConnection>();
            }
        }
        if must_wait {
            self.commands
                .entity(connection)
                .insert((profile, LoginState::AwaitingDeparture));
        } else {
            self.replies.finish(con, &profile);
            self.commands
                .entity(connection)
                .insert((profile, LoginState::Accepted));
        }
    }
}

pub(crate) fn handle_hello_packet(
    event: On<Received<ServerboundHello<'static>>>,
    mut query: Query<&mut ServerSideConnection, (Without<LoginState>, Without<AwaitingForwarding>)>,
    forwarding: Option<Res<ProxyForwarding>>,
    mut admission: Admission,
) {
    let (Ok(mut con), Some(pkt)) = (query.get_mut(event.entity), event.packet()) else {
        return;
    };
    if !is_valid_player_name(&pkt.username) {
        tracing::info!(name = ?pkt.username.0, "login refused: invalid characters in username");
        admission.refuse(event.entity, &mut con, invalid_player_name_reason());
        return;
    }
    if forwarding.is_some() {
        con.write_packet(&forwarding_query());
        admission
            .commands
            .entity(event.entity)
            .insert(AwaitingForwarding);
        return;
    }
    let profile = hello_profile(&pkt.username, admission.join_check.host.as_deref());
    admission.admit(event.entity, &mut con, profile);
}

pub(crate) fn handle_custom_query_answer(
    event: On<Received<ServerboundCustomQueryAnswer<'static>>>,
    mut query: Query<(&mut ServerSideConnection, Has<AwaitingForwarding>), Without<LoginState>>,
    forwarding: Option<Res<ProxyForwarding>>,
    mut admission: Admission,
) {
    let connection = event.entity;
    let Ok((mut con, awaiting)) = query.get_mut(connection) else {
        if let Ok((mut con, &state, &login, _)) = admission.session_connections.get_mut(connection)
        {
            disconnect(
                &mut con,
                client_protocol(state, Some(&login)),
                ForwardingRefusal::UnexpectedAnswer.reason(),
            );
            admission
                .commands
                .entity(connection)
                .remove::<ServerSideConnection>();
        }
        return;
    };
    let checked = match (awaiting, forwarding, event.packet()) {
        (true, Some(forwarding), Some(answer)) => forwarding.0.check(&answer),
        _ => Err(ForwardingRefusal::UnexpectedAnswer),
    };
    let player = match checked {
        Ok(player) => player,
        Err(refusal) => {
            tracing::info!(?refusal, "login refused: forwarding answer");
            admission.refuse(connection, &mut con, refusal.reason());
            return;
        }
    };
    let port = con.remote_addr().port();
    let forwarded = SocketAddr::new(player.address, port);
    if con.raw.remote_addr != forwarded {
        con.raw.remote_addr = forwarded;
    }
    admission
        .commands
        .entity(connection)
        .remove::<AwaitingForwarding>();
    let profile = GameProfile {
        id: player.id,
        username: player.name,
        properties: player.properties,
    };
    admission.admit(connection, &mut con, profile);
}

fn slow_login_reason() -> Text {
    Text::translate("multiplayer.disconnect.slow_login", Vec::new())
}

fn end_slow_logins(
    mut logins: Query<(
        Entity,
        &mut ServerSideConnection,
        &ConnectionState,
        &LoginStarted,
        Option<&LoginState>,
        Option<&GameProfile>,
    )>,
    departing: Query<(Entity, &Departing)>,
    ticks: Res<ServerTicks>,
    mut commands: Commands,
) {
    for (connection, mut con, &state, started, login, profile) in &mut logins {
        if state != ConnectionState::Login || ticks.0 - started.0 <= MAX_TICKS_BEFORE_LOGIN {
            continue;
        }
        tracing::info!(name = ?profile.map(|profile| &profile.username), "login timed out");
        disconnect(&mut con, client_protocol(state, login), slow_login_reason());
        commands.entity(connection).remove::<ServerSideConnection>();
        let (Some(LoginState::AwaitingDeparture), Some(profile)) = (login, profile) else {
            continue;
        };
        for (record, departing) in &departing {
            if departing.id == profile.id {
                tracing::warn!(
                    dim = ?departing.dim,
                    session = ?departing.session,
                    "a dimension never confirmed saving a departed player; no longer waiting for it"
                );
                commands.entity(record).despawn();
            }
        }
    }
}

fn finish_awaiting_logins(
    mut logins: Query<
        (Entity, &mut ServerSideConnection, &GameProfile, &LoginState),
        With<LoginStarted>,
    >,
    states: Query<&ConnectionState>,
    sessions: SessionsById,
    replies: LoginReplies,
    mut commands: Commands,
) {
    for (connection, mut con, profile, state) in &mut logins {
        let in_play = |connection| states.get(connection) == Ok(&ConnectionState::Game);
        if *state == LoginState::AwaitingDeparture && !sessions.in_world(profile.id, None, in_play)
        {
            replies.finish(&mut con, profile);
            commands.entity(connection).insert(LoginState::Accepted);
        }
    }
}

pub(crate) fn handle_login_acknowledged(
    event: On<Received<ServerboundLoginAcknowledged>>,
    mut query: Query<(&mut ServerSideConnection, &LoginState)>,
    mut commands: Commands,
) {
    let Ok((mut con, login)) = query.get_mut(event.entity) else {
        return;
    };
    if LoginState::Accepted != *login {
        return;
    }
    con.raw.login_acknowledged();
    commands
        .entity(event.entity)
        .remove::<LoginStarted>()
        .insert(ConnectionState::Configuration);
}

pub(crate) fn on_login_accepted(
    trigger: On<Insert<LoginState>>,
    login_state: Query<(&LoginState, &GameProfile)>,
    mut session_counter: ResMut<PlayerSessionCounter>,
    mut commands: Commands,
) {
    let connection_entity = trigger.event().entity;
    let Ok((state, profile)) = login_state.get(connection_entity) else {
        return;
    };
    if *state != LoginState::Accepted {
        return;
    }

    let host_anchor = commands
        .spawn((profile.clone(), SessionBundle::new(session_counter.next())))
        .id();
    commands
        .entity(connection_entity)
        .insert(HostAnchorRef(host_anchor));
}

#[cfg(test)]
mod tests {
    use super::is_valid_player_name;

    #[test]
    fn a_player_name_is_printable_ascii_other_than_space() {
        for accepted in ["!", "~", "Steve_01", "!~"] {
            assert!(is_valid_player_name(accepted), "{accepted:?}");
        }
        for refused in [
            " ", "St eve", "\u{7f}", "\u{0}", "\n", "Stеve", "§cSteve", "Ünal",
        ] {
            assert!(!is_valid_player_name(refused), "{refused:?}");
        }
    }
}
