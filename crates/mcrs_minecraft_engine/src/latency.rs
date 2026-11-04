//! The latency of a player, which the host measures on the wire and hands to the dimension of
//! the player for its player list entry.

use crate::bus::PlayerLatency;
use crate::channel_types::ToDim;
use crate::dim_inputs::DimInputs;
use crate::session::HostAnchorRef;
use bevy_ecs::component::Component;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Changed, Commands, Query, Res, ResMut};
use mcrs_minecraft_engine_world::session::{DimPlayerIndex, Session, SessionPlacement};

/// The smoothed round trip of the keep-alives of a connection in milliseconds. The host keeps it
/// on the connection; a dimension keeps the last value it received on the player.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Latency(pub i32);

impl Latency {
    /// Folds a new round trip in as the reference does: a quarter of the new value.
    #[must_use]
    pub fn sample(self, round_trip_millis: i32) -> Self {
        Self(self.0.saturating_mul(3).saturating_add(round_trip_millis) / 4)
    }
}

/// Host system: hands each latency that changed to the dimension of its session.
pub fn forward_latency(
    connections: Query<(&Latency, &HostAnchorRef), Changed<Latency>>,
    sessions: Query<(&Session, &SessionPlacement)>,
    mut inputs: ResMut<DimInputs>,
) {
    for (&Latency(latency), &HostAnchorRef(anchor)) in &connections {
        let Ok((&Session(session), placement)) = sessions.get(anchor) else {
            continue;
        };
        if let Some(dim) = placement.place().attached() {
            inputs.send_control(dim, ToDim::Latency(PlayerLatency { session, latency }));
        }
    }
}

/// Dimension system: keeps the latency that the host sent on the player of its session.
pub fn apply_latency(
    mut reader: MessageReader<PlayerLatency>,
    index: Res<DimPlayerIndex>,
    mut commands: Commands,
) {
    for message in reader.read() {
        if let Some(&entity) = index.0.get(&message.session)
            && let Ok(mut player) = commands.get_entity(entity)
        {
            player.try_insert(Latency(message.latency));
        }
    }
}
