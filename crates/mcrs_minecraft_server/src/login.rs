use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::{Add, Insert};
use bevy_ecs::prelude::{On, Query};
use bevy_ecs::query::{With, Without};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::any_with_component;
use bevy_ecs::system::{Commands, Res, ResMut, SystemParam};
use mcrs_minecraft_network::client::offline_player_uuid;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::packets::configuration;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundDisconnect;
use mcrs_minecraft_protocol::packets::login::clientbound::{
    ClientboundLoginDisconnect, ClientboundLoginFinished,
};
use mcrs_minecraft_protocol::packets::login::serverbound::{
    ServerboundHello, ServerboundLoginAcknowledged,
};
use mcrs_minecraft_protocol::profile::Property;
use mcrs_minecraft_protocol::{Bounded, Text, WritePacket, uuid};
use std::borrow::Cow;

use crate::disconnect::Departing;
use crate::world::session::{HostAnchorRef, SessionBundle, SessionConnection};
use mcrs_minecraft_level::session::{Place, PlayerSessionCounter, Session, SessionPlacement};

/// Vanilla mints one chat session id per listener and reuses it for every login.
#[derive(Resource, Clone, Copy, Debug)]
pub struct ChatSessionId(pub uuid::Uuid);

pub struct LoginPlugin;

impl bevy_app::Plugin for LoginPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.insert_resource(ChatSessionId(uuid::Uuid::new_v4()));
        app.init_resource::<ServerTicks>();
        app.add_systems(bevy_app::First, advance_server_ticks);
        app.add_observer(start_login_clock);
        app.add_observer(handle_hello_packet);
        app.add_observer(handle_login_acknowledged);
        app.add_observer(on_login_accepted);
        // A login ended as slow must be closed before a release in the same tick could finish it.
        app.add_systems(
            bevy_app::Update,
            (end_slow_logins, finish_awaiting_logins)
                .chain()
                .run_if(any_with_component::<LoginStarted>),
        );
    }
}

/// The game ends a login that is still pending after this many ticks.
const MAX_TICKS_BEFORE_LOGIN: u64 = 600;

/// Ticks the server has run, one per update.
#[derive(Resource, Default, Debug, Clone, Copy)]
struct ServerTicks(u64);

/// The tick on which the connection began its login, held until the client acknowledges it.
#[derive(Component, Debug, Clone, Copy)]
struct LoginStarted(u64);

fn advance_server_ticks(mut ticks: ResMut<ServerTicks>) {
    ticks.0 += 1;
}

fn start_login_clock(
    trigger: On<Add, ConnectionState>,
    states: Query<&ConnectionState>,
    ticks: Res<ServerTicks>,
    mut commands: Commands,
) {
    let connection = trigger.event().entity;
    if states.get(connection) == Ok(&ConnectionState::Login) {
        commands.entity(connection).insert(LoginStarted(ticks.0));
    }
}

#[derive(Debug, Default, Component, PartialEq, Eq, Clone, Copy)]
pub enum LoginState {
    #[default]
    Hello,
    /// Waiting for the sessions under its id to end and their players to be saved.
    AwaitingDeparture,
    Accepted,
}

#[derive(Debug, Clone, Component)]
pub struct GameProfile {
    pub id: uuid::Uuid,
    pub username: String,
    pub properties: Vec<Property<String>>,
}

impl<'a> From<&'a GameProfile> for mcrs_minecraft_protocol::profile::GameProfile<'a> {
    fn from(profile: &'a GameProfile) -> Self {
        let props: Vec<Property<&'a str>> = profile
            .properties
            .iter()
            .map(|p| Property {
                name: p.name.as_str(),
                value: p.value.as_str(),
                signature: p.signature.as_deref(),
            })
            .collect();

        Self {
            id: profile.id,
            username: Bounded::from(profile.username.as_str()),
            properties: Cow::Owned(props),
        }
    }
}

/// The player hosting an integrated server.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct SingleplayerProfile {
    pub name: String,
    pub id: uuid::Uuid,
}

/// The server authenticates nobody, so the id a hello carries is never trusted: a hello under
/// the host's name, in any case, plays as the host, and any other plays under the offline id of
/// its name.
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

// The game refuses such a name by throwing, and its connection sends the exception as the reason.
fn invalid_player_name_reason() -> Text {
    Text::translate(
        "disconnect.genericReason",
        [Text::text(
            "Internal Exception: java.lang.IllegalStateException: Invalid characters in username",
        )],
    )
}

pub fn duplicate_login_reason() -> Text {
    Text::translate("multiplayer.disconnect.duplicate_login", Vec::new())
}

/// The protocol the client decodes. It switches on receiving the packet that ends a phase, while
/// the server's state follows only on the client's acknowledgement.
fn client_protocol(state: ConnectionState, login: Option<&LoginState>) -> ConnectionState {
    match state {
        ConnectionState::Login if login == Some(&LoginState::Accepted) => {
            ConnectionState::Configuration
        }
        state => state,
    }
}

/// Writes the disconnect packet of the protocol the client decodes; the caller closes the
/// connection.
pub fn disconnect(con: &mut ServerSideConnection, state: ConnectionState, reason: Text) {
    match state {
        ConnectionState::Login => {
            let reason =
                serde_json::to_string(&reason).expect("a text component serializes to JSON");
            con.write_packet(&ClientboundLoginDisconnect {
                reason: Bounded(&reason),
            });
        }
        ConnectionState::Configuration => {
            con.write_packet(&configuration::ClientboundDisconnect { reason })
        }
        ConnectionState::Game => con.write_packet(&ClientboundDisconnect { reason }),
    }
}

/// The sessions on this server, and the players still being saved, by the id they play under.
#[derive(SystemParam)]
pub struct SessionsById<'w, 's> {
    anchors: Query<
        'w,
        's,
        (
            Entity,
            &'static GameProfile,
            &'static SessionPlacement,
            Option<&'static SessionConnection>,
        ),
        With<Session>,
    >,
    departing: Query<'w, 's, &'static Departing>,
}

impl SessionsById<'_, '_> {
    fn of(&self, id: uuid::Uuid) -> impl Iterator<Item = (Entity, Place, Option<Entity>)> {
        self.anchors
            .iter()
            .filter(move |(_, profile, ..)| profile.id == id)
            .map(|(anchor, _, placement, connection)| {
                (
                    anchor,
                    placement.place(),
                    connection.map(SessionConnection::entity),
                )
            })
    }

    fn saving(&self, id: uuid::Uuid) -> bool {
        self.departing.iter().any(|departing| departing.id == id)
    }

    /// Whether a player under `id`, other than the session on `own`, is in the world: placed in
    /// a dimension, in play, or not yet saved.
    pub fn in_world(
        &self,
        id: uuid::Uuid,
        own: Option<Entity>,
        in_play: impl Fn(Entity) -> bool,
    ) -> bool {
        self.saving(id)
            || self.of(id).any(|(anchor, place, connection)| {
                Some(anchor) != own
                    && (place != Place::Unplaced || connection.is_some_and(&in_play))
            })
    }
}

fn finish_login(con: &mut ServerSideConnection, profile: &GameProfile, session_id: uuid::Uuid) {
    con.write_packet(&ClientboundLoginFinished {
        profile: profile.into(),
        session_id,
    });
}

pub fn handle_hello_packet(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(&mut ServerSideConnection, &ConnectionState), Without<LoginState>>,
    mut session_connections: Query<(&mut ServerSideConnection, &ConnectionState), With<LoginState>>,
    sessions: SessionsById,
    session_id: Res<ChatSessionId>,
    host: Option<Res<SingleplayerProfile>>,
    mut commands: Commands,
) {
    let Ok((mut con, state)) = query.get_mut(event.entity) else {
        return;
    };
    if ConnectionState::Login != *state {
        return;
    }
    let Some(pkt) = event.decode::<ServerboundHello>() else {
        return;
    };
    if !is_valid_player_name(&pkt.username) {
        tracing::info!(name = ?pkt.username.0, "login refused: invalid characters in username");
        disconnect(
            &mut con,
            ConnectionState::Login,
            invalid_player_name_reason(),
        );
        commands
            .entity(event.entity)
            .remove::<ServerSideConnection>();
        return;
    }
    let profile = hello_profile(&pkt.username, host.as_deref());
    tracing::debug!(?profile, "login hello");
    let mut must_wait = sessions.saving(profile.id);
    for (_, place, connection) in sessions.of(profile.id) {
        let other = connection.and_then(|entity| {
            let (con, &state) = session_connections.get_mut(entity).ok()?;
            Some((entity, con, state))
        });
        let in_play = matches!(other, Some((_, _, ConnectionState::Game)));
        if place == Place::Unplaced && !in_play {
            continue;
        }
        must_wait = true;
        if let Some((connection, mut other, state)) = other {
            tracing::info!(name = %profile.username, "a new login under the same id ends the session");
            disconnect(&mut other, state, duplicate_login_reason());
            commands.entity(connection).remove::<ServerSideConnection>();
        }
    }
    if must_wait {
        commands
            .entity(event.entity)
            .insert((profile, LoginState::AwaitingDeparture));
    } else {
        finish_login(&mut con, &profile, session_id.0);
        commands
            .entity(event.entity)
            .insert((profile, LoginState::Accepted));
    }
}

fn slow_login_reason() -> Text {
    Text::translate("multiplayer.disconnect.slow_login", Vec::new())
}

/// A held login that runs out of time also gives up on the saves it was waiting for, or every
/// later login under its id would wait for them too.
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
    session_id: Res<ChatSessionId>,
    mut commands: Commands,
) {
    for (connection, mut con, profile, state) in &mut logins {
        let in_play = |connection| states.get(connection) == Ok(&ConnectionState::Game);
        if *state == LoginState::AwaitingDeparture && !sessions.in_world(profile.id, None, in_play)
        {
            finish_login(&mut con, profile, session_id.0);
            commands.entity(connection).insert(LoginState::Accepted);
        }
    }
}

pub fn handle_login_acknowledged(
    event: On<ReceivedPacketEvent>,
    query: Query<(&ConnectionState, &LoginState)>,
    mut commands: Commands,
) {
    let Ok((state, login)) = query.get(event.entity) else {
        return;
    };
    if ConnectionState::Login != *state || LoginState::Accepted != *login {
        return;
    }
    let Some(_) = event.decode::<ServerboundLoginAcknowledged>() else {
        return;
    };
    commands
        .entity(event.entity)
        .remove::<LoginStarted>()
        .insert(ConnectionState::Configuration);
}

pub fn on_login_accepted(
    trigger: On<Insert, LoginState>,
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

    #[test]
    fn a_player_name_holds_at_most_sixteen_characters() {
        assert!(is_valid_player_name(&"a".repeat(16)));
        assert!(!is_valid_player_name(&"a".repeat(17)));
    }

    #[test]
    fn an_empty_player_name_is_valid_as_in_the_game() {
        assert!(is_valid_player_name(""));
    }
}
