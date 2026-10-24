#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "the panics in this crate are not yet replaced by errors or by code that cannot panic"
)]
#[cfg(target_family = "wasm")]
pub mod browser;
#[cfg(feature = "bevy")]
pub mod client;
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
pub mod connect;
#[cfg(feature = "bevy")]
pub mod event;
pub mod identity;
#[cfg(feature = "bevy")]
pub mod inbound_rate;
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
mod intent;
pub mod join;
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
pub mod lan;
#[cfg(feature = "bevy")]
pub mod metrics;
pub mod packet_io;
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
pub mod webtransport;

/// Reading a `std::time::Instant` panics in the browser, so packet timestamps
/// come from `performance.now()` there instead.
#[cfg(not(target_family = "wasm"))]
pub use std::time::Instant;
#[cfg(target_family = "wasm")]
pub use web_time::Instant;

pub use crate::packet_io::{MAX_QUEUED_BYTES_PER_SOCKET, RawConnection};

/// System sets for the network layer, usable for ordering constraints in
/// downstream crates. `SpawnConnections` contains `spawn_new_raw_connections`.
/// Other crates should schedule their connection-setup systems
/// `.after(NetworkSet::SpawnConnections)` in `FixedPreUpdate`.
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, bevy_ecs::schedule::SystemSet)]
pub enum NetworkSet {
    SpawnConnections,
}
use bytes::Bytes;

#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
pub(crate) use server::SharedNetworkState;
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
mod server;
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
pub use server::{BoundAddress, NetworkPlugin, ServerSideConnection, WebTransportEndpoint};

#[derive(Clone, Debug)]
pub struct ReceivedPacket {
    pub timestamp: Instant,
    pub id: i32,
    pub payload: Bytes,
}

/// When each side of a login switches compression on, fixed by the protocol version of the
/// handshake.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SwitchMode {
    /// Each side switches both directions as it writes or reads the compression packet.
    Immediate,
    /// The client switches as it reads `login_finished`, and the server as it reads the
    /// `login_acknowledged` that answers it.
    Acknowledged,
}

pub const PROTOCOL_26_3: i32 = 777;
/// The first protocol version that switches compression on acknowledgement.
pub const PROTOCOL_26_4_SNAPSHOT_5: i32 = 1_073_742_167;

impl SwitchMode {
    pub fn of_own_version() -> Self {
        Self::of(mcrs_minecraft_core::VERSION.protocol_version).unwrap_or(Self::Immediate)
    }

    /// The mode of each protocol version a login accepts; another version is refused.
    pub fn of(protocol_version: i32) -> Option<Self> {
        match protocol_version {
            PROTOCOL_26_3 => Some(Self::Immediate),
            PROTOCOL_26_4_SNAPSHOT_5 => Some(Self::Acknowledged),
            own if own == mcrs_minecraft_core::VERSION.protocol_version => Some(Self::Immediate),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
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
    for (byte, pair) in hash.iter_mut().zip(hex.as_bytes().as_chunks::<2>().0) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair)?, 16)?;
    }
    Ok(hash)
}

#[cfg(all(test, feature = "bevy", not(target_family = "wasm")))]
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
