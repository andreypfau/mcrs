use bevy_ecs::component::Component;
use bevy_ecs::prelude::{On, Query};
use bevy_ecs::query::{With, Without};
use bevy_ecs::system::Commands;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_network::{ConnectionState, ServerSideConnection};
use mcrs_protocol::packets::login::clientbound::ClientboundLoginFinished;
use mcrs_protocol::packets::login::serverbound::{ServerboundHello, ServerboundLoginAcknowledged};
use mcrs_protocol::profile::Property;
use mcrs_protocol::{Bounded, WritePacket, uuid};
use std::borrow::Cow;

pub struct LoginPlugin;

impl bevy_app::Plugin for LoginPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_observer(handle_hello_packet);
        app.add_observer(handle_login_acknowledged);
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
