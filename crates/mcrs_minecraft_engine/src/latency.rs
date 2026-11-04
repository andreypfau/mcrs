//! The latency of a player, which the host measures on the wire and hands to the muxer for its
//! player list entry.

use crate::entity_replication::EntityReplication;
use crate::session::HostAnchorRef;
use bevy_ecs::component::Component;
use bevy_ecs::prelude::{Changed, Query, ResMut};
use mcrs_minecraft_engine_world::session::Session;

/// The smoothed round trip of the keep-alives of a connection in milliseconds.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Latency(pub i32);

impl Latency {
    /// Folds a new round trip in as the reference does: a quarter of the new value.
    #[must_use]
    pub fn sample(self, round_trip_millis: i32) -> Self {
        Self(self.0.saturating_mul(3).saturating_add(round_trip_millis) / 4)
    }
}

/// Host system: hands each latency that changed to the muxer.
pub fn forward_latency(
    connections: Query<(&Latency, &HostAnchorRef), Changed<Latency>>,
    sessions: Query<&Session>,
    replication: Option<ResMut<EntityReplication>>,
) {
    let Some(mut replication) = replication else {
        return;
    };
    for (&Latency(latency), &HostAnchorRef(anchor)) in &connections {
        if let Ok(&Session(session)) = sessions.get(anchor) {
            replication.send_latency(session, latency);
        }
    }
}
