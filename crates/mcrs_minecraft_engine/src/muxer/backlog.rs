use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Local, Populated, Query, ResMut};
use mcrs_minecraft_engine_world::session::Session;
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_replication::Backlog;
use rustc_hash::FxHashSet;
use tracing::debug;

use crate::muxer::muxer_link::MuxerLink;
use crate::muxer::session::HostAnchorRef;

const BEHIND_BYTES: usize = 512 * 1024;

const CLEAR_BYTES: usize = 64 * 1024;

pub fn forward_backlog(
    connections: Populated<(Entity, &ServerSideConnection, &HostAnchorRef)>,
    sessions: Query<&Session>,
    muxer: Option<ResMut<MuxerLink>>,
    mut behind: Local<FxHashSet<Entity>>,
) {
    let Some(mut muxer) = muxer else {
        return;
    };
    behind.retain(|connection| connections.contains(*connection));
    for (connection, server_side, &HostAnchorRef(anchor)) in connections.iter() {
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
            debug!(
                ?session,
                ?backlog,
                unsent,
                "the backlog of a connection changed"
            );
            muxer.send_backlog(session, backlog);
        }
    }
}
