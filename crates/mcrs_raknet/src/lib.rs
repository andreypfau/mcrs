#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod ack;
mod connected;
mod datagram;
mod fragment;
mod frame;
mod magic;
mod reliability;
mod server;
mod session;
mod unconnected;
mod wire;

#[cfg(feature = "tokio")]
mod driver;

pub use ack::{AckPacket, AckRecord, MAX_RECORDS_PER_ACK};
pub use connected::{
    ConnectedPing, ConnectedPong, ConnectionRequest, ConnectionRequestAccepted,
    NewIncomingConnection, SYSTEM_ADDRESSES,
};
pub use datagram::Datagram;
pub use fragment::{
    DEFAULT_SPLIT_TIMEOUT_MS, FragmentError, FragmentReassembler, MAX_ACTIVE_SPLITS,
    MAX_SPLIT_COUNT, MAX_TOTAL_REASSEMBLY_BYTES,
};
pub use frame::{Frame, MAX_UNSPLIT_PAYLOAD, SplitInfo};
pub use magic::{RAKNET_MAGIC, is_valid_magic};
pub use reliability::Reliability;
pub use server::{MAX_SESSIONS, RaknetEvent, RaknetServer, TransportEvent};
pub use session::{
    MAX_CHANNEL_BUFFER_PER_CHANNEL, MAX_IN_FLIGHT_DATAGRAMS, MAX_MTU, MAX_ORDER_CHANNELS,
    MAX_ORDER_WINDOW, MAX_PENDING_SEQS, MAX_RETRANSMIT_ATTEMPTS, MIN_MTU, RaknetSession,
    SessionError, SessionState,
};
pub use unconnected::{
    Motd, OpenConnectionReply1, OpenConnectionReply2, OpenConnectionRequest1,
    OpenConnectionRequest2, UnconnectedPing, UnconnectedPong, id as packet_id,
};
pub use wire::{AddressCodec, Uint24};

#[cfg(feature = "tokio")]
pub use driver::{DriverError, RaknetListener};
