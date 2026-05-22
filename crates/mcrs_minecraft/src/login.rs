use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Add;
use bevy_ecs::prelude::{On, Query};
use bevy_ecs::query::{With, Without};
use bevy_ecs::system::{Commands, ResMut};
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_network::{ConnectionState, ServerSideConnection};
use mcrs_protocol::packets::login::clientbound::ClientboundLoginFinished;
use mcrs_protocol::packets::login::serverbound::{ServerboundHello, ServerboundLoginAcknowledged};
use mcrs_protocol::profile::Property;
use mcrs_protocol::{Bounded, WritePacket, uuid};
use smallvec::SmallVec;
use std::borrow::Cow;

use crate::world::player_index::{HostAnchorRef, PlayerIndex, PlayerLocation};

pub struct LoginPlugin;

impl bevy_app::Plugin for LoginPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_observer(handle_hello_packet);
        app.add_observer(handle_login_acknowledged);
        app.add_observer(on_login_accepted);
    }
}

#[derive(Debug, Default, Component, PartialEq, Eq, Clone, Copy)]
pub enum LoginState {
    #[default]
    Hello,
    Key,
    Authenticating,
    Negotiating,
    Verifying,
    WaitingForDupeDisconnect,
    ProtocolSwitching,
    Accepted,
}

#[derive(Debug, Clone, Component)]
pub struct GameProfile {
    pub id: uuid::Uuid,
    pub username: String,
    pub properties: Vec<Property<String>>,
}

impl<'a> From<&'a GameProfile> for mcrs_protocol::profile::GameProfile<'a> {
    fn from(profile: &'a GameProfile) -> Self {
        // сконвертировать вектор свойств в Property<&str>
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
            username: Bounded::try_from(profile.username.as_str())
                .expect("username longer than 16 chars"),
            properties: Cow::Owned(props),
        }
    }
}

pub fn handle_hello_packet(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(&mut ServerSideConnection, &ConnectionState), Without<LoginState>>,
    mut commands: Commands,
) {
    let Ok((mut con, state)) = query.get_mut(event.entity) else {
        return;
    };
    if ConnectionState::Login != *state {
        return;
    }
    println!("handle_hello_packet: {:?}", event.data);
    let Some(pkt) = event.decode::<ServerboundHello>() else {
        return;
    };
    let profile = GameProfile {
        id: pkt.profile_id,
        username: pkt.username.to_string(),
        properties: Vec::new(),
    };
    println!("new profile: {:?}", profile);
    let response = ClientboundLoginFinished {
        profile: (&profile).into(),
    };
    con.write_packet(&response);
    commands
        .entity(event.entity)
        .insert((profile, LoginState::Accepted));
}

pub fn handle_login_acknowledged(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(&ConnectionState), With<LoginState>>,
    mut commands: Commands,
) {
    let Ok((state)) = query.get_mut(event.entity) else {
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
    mut player_index: ResMut<PlayerIndex>,
    mut commands: Commands,
) {
    let connection_entity = trigger.event().entity;
    let Ok((state, profile)) = login_state.get(connection_entity) else {
        return;
    };
    if *state != LoginState::Accepted {
        return;
    }

    // current_dim = PLACEHOLDER until dim selection from spawn-point logic lands.
    let host_anchor = commands.spawn(profile.clone()).id();

    commands
        .entity(connection_entity)
        .insert(HostAnchorRef(host_anchor));

    player_index.insert(
        host_anchor,
        PlayerLocation {
            socket: connection_entity,
            current_dim: Entity::PLACEHOLDER,
            previous_dim: None,
            in_dim_entity: None,
            inbound_pending: SmallVec::new(),
        },
    );
}
