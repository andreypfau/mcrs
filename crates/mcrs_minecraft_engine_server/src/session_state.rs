use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::{Query, SystemParam};
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::muxer::disconnect::Departing;
use mcrs_minecraft_engine::muxer::session::SessionConnection;
use mcrs_minecraft_engine_world::session::{Place, Session, SessionPlacement};
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::uuid;
use mcrs_minecraft_text::Text;

#[derive(Debug, Default, Component, PartialEq, Eq, Clone, Copy)]
pub enum LoginState {
    #[default]
    Hello,
    AwaitingDeparture,
    Accepted,
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub(crate) struct AwaitingConfigurationAck;

pub(crate) fn duplicate_login_reason() -> Text {
    Text::translate("multiplayer.disconnect.duplicate_login", Vec::new())
}

pub(crate) fn client_protocol(
    state: ConnectionState,
    login: Option<&LoginState>,
) -> ConnectionState {
    match state {
        ConnectionState::Login if login == Some(&LoginState::Accepted) => {
            ConnectionState::Configuration
        }
        state => state,
    }
}

pub(crate) fn disconnect(con: &mut ServerSideConnection, state: ConnectionState, reason: Text) {
    if let Err(error) = con.disconnect(state, reason) {
        tracing::warn!("a disconnect could not be encoded: {error:#}");
    }
}

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
    pub(crate) fn of(
        &self,
        id: uuid::Uuid,
    ) -> impl Iterator<Item = (Entity, Place, Option<Entity>)> {
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

    pub(crate) fn saving(&self, id: uuid::Uuid) -> bool {
        self.departing.iter().any(|departing| departing.id == id)
    }

    pub(crate) fn in_world(
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
