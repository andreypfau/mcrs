use bevy_ecs::component::Component;
use bevy_ecs::lifecycle::Add;
use bevy_ecs::prelude::{On, Query};
use bevy_ecs::query::Without;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Commands, Res, ResMut};
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::packets::login::clientbound::ClientboundLoginFinished;
use mcrs_minecraft_protocol::packets::login::serverbound::{
    ServerboundHello, ServerboundLoginAcknowledged,
};
use mcrs_minecraft_protocol::profile::Property;
use mcrs_minecraft_protocol::{Bounded, WritePacket, uuid};
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

pub fn handle_hello_packet(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(&mut ServerSideConnection, &ConnectionState), Without<LoginState>>,
    session_id: Res<ChatSessionId>,
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
    let profile = GameProfile {
        id: pkt.profile_id,
        username: pkt.username.to_string(),
        properties: Vec::new(),
    };
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
