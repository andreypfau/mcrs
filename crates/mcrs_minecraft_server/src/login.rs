use bevy_ecs::component::Component;
use bevy_ecs::lifecycle::Add;
use bevy_ecs::prelude::{On, Query};
use bevy_ecs::query::Without;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Commands, Res, ResMut};
use mcrs_minecraft_network::client::offline_player_uuid;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::packets::login::clientbound::{
    ClientboundLoginDisconnect, ClientboundLoginFinished,
};
use mcrs_minecraft_protocol::packets::login::serverbound::{
    ServerboundHello, ServerboundLoginAcknowledged,
};
use mcrs_minecraft_protocol::profile::Property;
use mcrs_minecraft_protocol::{Bounded, Text, WritePacket, uuid};
use std::borrow::Cow;

use crate::world::session::{HostAnchorRef, SessionBundle};
use mcrs_minecraft_level::session::PlayerSessionCounter;

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
    }
}

#[derive(Debug, Default, Component, PartialEq, Eq, Clone, Copy)]
pub enum LoginState {
    #[default]
    Hello,
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

fn refuse_login(con: &mut ServerSideConnection, reason: &Text) {
    let reason = serde_json::to_string(reason).expect("a text component serializes to JSON");
    con.write_packet(&ClientboundLoginDisconnect {
        reason: Bounded(&reason),
    });
}

pub fn handle_hello_packet(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(&mut ServerSideConnection, &ConnectionState), Without<LoginState>>,
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
        refuse_login(&mut con, &invalid_player_name_reason());
        commands
            .entity(event.entity)
            .remove::<ServerSideConnection>();
        return;
    }
    let profile = hello_profile(&pkt.username, host.as_deref());
    tracing::debug!(?profile, "login hello");
    let response = ClientboundLoginFinished {
        profile: (&profile).into(),
        session_id: session_id.0,
    };
    con.write_packet(&response);
    commands
        .entity(event.entity)
        .insert((profile, LoginState::Accepted));
}

pub fn handle_login_acknowledged(
    event: On<ReceivedPacketEvent>,
    mut query: Query<&ConnectionState, With<LoginState>>,
    mut commands: Commands,
) {
    let Ok(state) = query.get_mut(event.entity) else {
        return;
    };
    if ConnectionState::Login != *state {
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
    trigger: On<Add, LoginState>,
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

use bevy_ecs::query::With;

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
