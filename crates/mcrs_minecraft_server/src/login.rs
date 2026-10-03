use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Insert;
use bevy_ecs::prelude::{On, Query};
use bevy_ecs::query::{With, Without};
use bevy_ecs::resource::Resource;
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
        app.add_observer(handle_hello_packet);
        app.add_observer(handle_login_acknowledged);
        app.add_observer(on_login_accepted);
        app.add_systems(bevy_app::Update, finish_awaiting_logins);
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
pub fn hello_profile(username: &str, host: Option<&SingleplayerProfile>) -> GameProfile {
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

/// Writes the disconnect packet of the connection's protocol state; the caller closes it.
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

    /// A login under `id` waits while a session under it lives or its player is not yet saved.
    pub fn must_wait(&self, id: uuid::Uuid) -> bool {
        self.of(id).next().is_some() || self.saving(id)
    }

    /// Whether a player under `id`, other than the session on `own`, is in the world: placed in
    /// a dimension, in play, or not yet saved.
    pub fn in_world(&self, id: uuid::Uuid, own: Entity, in_play: impl Fn(Entity) -> bool) -> bool {
        self.saving(id)
            || self.of(id).any(|(anchor, place, connection)| {
                anchor != own && (place != Place::Unplaced || connection.is_some_and(&in_play))
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
    let must_wait = sessions.must_wait(profile.id);
    for (_, _, connection) in sessions.of(profile.id) {
        let Some(connection) = connection else {
            continue;
        };
        let Ok((mut other, &state)) = session_connections.get_mut(connection) else {
            continue;
        };
        tracing::info!(name = %profile.username, "a new login under the same id ends the session");
        disconnect(&mut other, state, duplicate_login_reason());
        commands.entity(connection).remove::<ServerSideConnection>();
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

pub fn finish_awaiting_logins(
    mut logins: Query<(Entity, &mut ServerSideConnection, &GameProfile, &LoginState)>,
    sessions: SessionsById,
    session_id: Res<ChatSessionId>,
    mut commands: Commands,
) {
    for (connection, mut con, profile, state) in &mut logins {
        if *state == LoginState::AwaitingDeparture && !sessions.must_wait(profile.id) {
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
