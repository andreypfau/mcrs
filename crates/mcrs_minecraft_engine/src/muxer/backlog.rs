//! The backlog of each connection, which the host measures on the writer and hands to the muxer,
//! so that a client that reads slowly gets fewer and fresher mob moves instead of a queue that
//! grows until the server kicks it.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Local, Query, ResMut};
use mcrs_minecraft_engine_world::session::Session;
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_replication::Backlog;
use rustc_hash::FxHashSet;

use crate::muxer::muxer_link::MuxerLink;
use crate::muxer::session::HostAnchorRef;

/// The unsent bytes from which a connection is behind. One tick of the moves of 27,000 pigs is
/// about 40 KiB, so the muxer sees the backlog long before the stall limit of the bridge.
const BEHIND_BYTES: usize = 512 * 1024;

/// The unsent bytes below which a connection that was behind catches up again.
const CLEAR_BYTES: usize = 64 * 1024;

/// Host system: hands each change of the backlog of a connection to the muxer.
pub fn forward_backlog(
    connections: Query<(Entity, &ServerSideConnection, &HostAnchorRef)>,
    sessions: Query<&Session>,
    muxer: Option<ResMut<MuxerLink>>,
    mut behind: Local<FxHashSet<Entity>>,
) {
    let Some(mut muxer) = muxer else {
        return;
    };
    behind.retain(|connection| connections.contains(*connection));
    for (connection, server_side, &HostAnchorRef(anchor)) in &connections {
        let unsent = server_side.raw.unsent_bytes();
        let backlog = if behind.contains(&connection) {
            if unsent >= CLEAR_BYTES {
                continue;
            }
            behind.remove(&connection);
            Backlog::Clear
        } else {
            if unsent <= BEHIND_BYTES {
                continue;
            }
            behind.insert(connection);
            Backlog::Behind
        };
        if let Ok(&Session(session)) = sessions.get(anchor) {
            muxer.send_backlog(session, backlog);
        }
    }
}
