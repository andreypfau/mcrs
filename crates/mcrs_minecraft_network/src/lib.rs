#[cfg(target_family = "wasm")]
pub mod browser;
pub mod client;
#[cfg(not(target_family = "wasm"))]
pub mod connect;
pub mod event;
#[cfg(not(target_family = "wasm"))]
mod intent;
pub mod metrics;
pub mod packet_io;
#[cfg(not(target_family = "wasm"))]
mod status;
#[cfg(not(target_family = "wasm"))]
pub mod webtransport;

/// Reading a `std::time::Instant` panics in the browser, so packet timestamps
/// come from `performance.now()` there instead.
#[cfg(not(target_family = "wasm"))]
pub use std::time::Instant;
#[cfg(target_family = "wasm")]
pub use web_time::Instant;

pub use crate::packet_io::{MAX_QUEUED_BYTES_PER_SOCKET, RawConnection};
use bevy_ecs::prelude::Component;

/// System sets for the network layer, usable for ordering constraints in
/// downstream crates. `SpawnConnections` contains `spawn_new_raw_connections`.
/// Other crates should schedule their connection-setup systems
/// `.after(NetworkSet::SpawnConnections)` in `FixedPreUpdate`.
#[cfg(not(target_family = "wasm"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, bevy_ecs::schedule::SystemSet)]
pub enum NetworkSet {
    SpawnConnections,
}
use bytes::Bytes;
use tokio::sync::mpsc::error::TryRecvError;

#[cfg(not(target_family = "wasm"))]
mod server;
#[cfg(not(target_family = "wasm"))]
pub(crate) use server::SharedNetworkState;
#[cfg(not(target_family = "wasm"))]
pub use server::{
    BoundAddress, InGameConnectionState, NetworkPlugin, ServerSideConnection, WebTransportEndpoint,
};

#[derive(Clone, Debug)]
pub struct ReceivedPacket {
    pub timestamp: Instant,
    pub id: i32,
    pub payload: Bytes,
}

#[derive(Debug, Component, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ConnectionState {
    Login,
    Configuration,
    Game,
}

/// The inverse of [`WebTransportEndpoint::certificate_hash_hex`]: a browser can
/// only reach the self-signed development endpoint by passing the hash back,
/// and it arrives as the hex the server printed.
pub fn certificate_hash_from_hex(hex: &str) -> anyhow::Result<[u8; 32]> {
    let hex = hex.trim();
    if hex.len() != 64 {
        anyhow::bail!(
            "a SHA-256 certificate hash is 64 hex digits, got {}",
            hex.len()
        );
    }
    let mut hash = [0u8; 32];
    for (byte, pair) in hash.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair)?, 16)?;
    }
    Ok(hash)
}

pub trait EngineConnection: Send + Sync + 'static {
    fn try_recv(&mut self) -> Result<Option<ReceivedPacket>, TryRecvError>;
    fn flush(&mut self) -> anyhow::Result<()>;
    fn queued_bytes(&self) -> usize;
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

    #[test]
    fn a_published_certificate_hash_round_trips_through_its_hex() {
        let endpoint = WebTransportEndpoint {
            address: "127.0.0.1:25565".parse().unwrap(),
            certificate_hash: std::array::from_fn(|i| (i * 7 + 3) as u8),
        };
        let parsed = certificate_hash_from_hex(&endpoint.certificate_hash_hex()).unwrap();
        assert_eq!(parsed, endpoint.certificate_hash);
    }

    #[test]
    fn a_truncated_certificate_hash_is_refused() {
        assert!(certificate_hash_from_hex("abcd").is_err());
        assert!(certificate_hash_from_hex(&"z".repeat(64)).is_err());
    }
}
