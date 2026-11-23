use crate::muxer::muxer_link::MuxerLink;
use crate::muxer::session::HostAnchorRef;
use bevy_ecs::component::Component;
use bevy_ecs::prelude::{Changed, Populated, Query, ResMut};
use mcrs_minecraft_engine_world::session::Session;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Latency(pub i32);

impl Latency {
    #[must_use]
    pub fn sample(self, round_trip_millis: i32) -> Self {
        Self(self.0.saturating_mul(3).saturating_add(round_trip_millis) / 4)
    }
}

pub fn forward_latency(
    connections: Populated<(&Latency, &HostAnchorRef), Changed<Latency>>,
    sessions: Query<&Session>,
    muxer: Option<ResMut<MuxerLink>>,
) {
    let Some(mut muxer) = muxer else {
        return;
    };
    for (&Latency(latency), &HostAnchorRef(anchor)) in &connections {
        if let Ok(&Session(session)) = sessions.get(anchor) {
            muxer.send_latency(session, latency);
        }
    }
}
