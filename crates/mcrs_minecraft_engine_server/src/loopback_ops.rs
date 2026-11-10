use bevy_ecs::lifecycle::Insert;
use bevy_ecs::prelude::{On, Query, Res, ResMut, Resource};
use mcrs_minecraft_engine::coordinator::ops::{OpList, OpListEntry};
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_network::ServerSideConnection;
use std::net::IpAddr;

/// Whether a player whose connection comes from a loopback address joins as an operator at the
/// top level. This is a convenience of this server for a local game; vanilla has no such rule.
/// The ops file does not hold such an operator.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoopbackOps {
    Grant,
    #[default]
    Deny,
}

/// Whether `address` is in 127.0.0.0/8 or is `::1`, also as an IPv4-mapped IPv6 address.
fn is_loopback(address: IpAddr) -> bool {
    address.to_canonical().is_loopback()
}

pub(crate) fn grant_loopback_ops(
    profile: On<Insert, GameProfile>,
    policy: Res<LoopbackOps>,
    connections: Query<(&ServerSideConnection, &GameProfile)>,
    mut ops: ResMut<OpList>,
) {
    if *policy != LoopbackOps::Grant {
        return;
    }
    let Ok((connection, profile)) = connections.get(profile.event().entity) else {
        return;
    };
    if !is_loopback(connection.remote_addr().ip()) {
        return;
    }
    if ops.grant_loopback(OpListEntry::owner(profile.id, profile.username.clone())) {
        tracing::info!(name = %profile.username, "a loopback join makes the player an operator");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_covers_the_v4_block_v6_and_mapped_v4_only() {
        for local in ["127.0.0.1", "127.255.0.9", "::1", "::ffff:127.0.0.1"] {
            assert!(is_loopback(local.parse().unwrap()), "{local}");
        }
        for remote in [
            "192.168.1.2",
            "10.0.0.1",
            "::",
            "0.0.0.0",
            "::ffff:10.0.0.1",
            "fe80::1",
        ] {
            assert!(!is_loopback(remote.parse().unwrap()), "{remote}");
        }
    }
}
