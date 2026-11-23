mod answer;
#[cfg(target_family = "wasm")]
pub mod browser;
#[cfg(feature = "bevy")]
pub mod client;
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
pub mod connect;
mod disconnect_reason;
#[cfg(feature = "bevy")]
pub mod event;
mod forwarding_secret;
pub mod identity;
mod inbound;
#[cfg(feature = "bevy")]
pub mod inbound_rate;
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
mod intent;
pub mod join;
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
pub mod lan;
#[cfg(feature = "bevy")]
pub mod metrics;
mod outbound;
pub mod packet_io;
mod ping_limits;
mod play_pings;
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
mod proxy_header;
mod shared_frames;
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
pub mod webtransport;
mod writer_signal;
#[cfg(not(target_family = "wasm"))]
mod writer_wakes;

#[cfg(not(target_family = "wasm"))]
pub use std::time::Instant;
#[cfg(target_family = "wasm")]
pub use web_time::Instant;

pub use crate::answer::Answer;
pub use crate::forwarding_secret::{
    EmptyForwardingSecret, FORWARDING_CHANNEL, ForwardedPlayer, ForwardingRefusal,
    ForwardingSecret, forwarding_query,
};
#[cfg(not(target_family = "wasm"))]
pub use crate::outbound::OutboundReceiver;
pub use crate::packet_io::{Inbound, MAX_QUEUED_BYTES_PER_SOCKET, RawConnection};
pub use crate::ping_limits::PingLimits;
pub use crate::play_pings::PlayPings;
pub use crate::shared_frames::{SharedAppend, SharedFrames};
#[cfg(not(target_family = "wasm"))]
pub use crate::writer_wakes::WriterWakes;
pub use mcrs_minecraft_protocol::ConnectionState;

#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, bevy_ecs::schedule::SystemSet)]
pub enum NetworkSet {
    SpawnConnections,
    RefreshStatus,
}
use bytes::Bytes;
use mcrs_minecraft_move_validate::Verdict;

#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
pub(crate) use server::SharedNetworkState;
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
mod server;
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
pub use server::{
    BoundAddress, NetworkPlugin, ServerSideConnection, StatusPlayers, StatusPublisher,
    WebTransportEndpoint, default_favicon,
};

#[derive(Clone, Debug)]
pub struct ReceivedPacket {
    pub timestamp: Instant,
    pub id: i32,
    pub payload: Bytes,
    pub verdict: Option<Verdict>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SwitchMode {
    Immediate,
    Acknowledged,
}

pub const PROTOCOL_26_3: i32 = 777;
pub const PROTOCOL_26_4_SNAPSHOT_5: i32 = 1_073_742_167;

impl SwitchMode {
    pub fn of_own_version() -> Self {
        Self::of(mcrs_minecraft_core::VERSION.protocol_version).unwrap_or(Self::Immediate)
    }

    pub fn of(protocol_version: i32) -> Option<Self> {
        match protocol_version {
            PROTOCOL_26_3 => Some(Self::Immediate),
            PROTOCOL_26_4_SNAPSHOT_5 => Some(Self::Acknowledged),
            own if own == mcrs_minecraft_core::VERSION.protocol_version => Some(Self::Acknowledged),
            _ => None,
        }
    }
}

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

#[cfg(test)]
#[cfg(all(feature = "bevy", not(target_family = "wasm")))]
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
