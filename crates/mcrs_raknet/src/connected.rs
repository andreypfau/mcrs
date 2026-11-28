extern crate alloc;

use core::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::unconnected::id;
use crate::wire::AddressCodec;

pub const SYSTEM_ADDRESSES: [SocketAddr; 10] = [
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0),
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionRequest {
    pub client_guid: u64,
    pub request_time: i64,
    pub security: bool,
}

impl ConnectionRequest {
    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        if *buf.first()? != id::CONNECTION_REQUEST {
            return None;
        }
        let guid_slice = buf.get(1..9)?;
        let time_slice = buf.get(9..17)?;
        let sec_byte = *buf.get(17)?;

        let client_guid = u64::from_be_bytes(guid_slice.try_into().ok()?);
        let request_time = i64::from_be_bytes(time_slice.try_into().ok()?);
        let security = sec_byte != 0;
        Some(Self {
            client_guid,
            request_time,
            security,
        })
    }

    pub fn write_to(&self, buf: &mut [u8]) -> Option<usize> {
        let needed = 18;
        if buf.len() < needed {
            return None;
        }
        if let Some(slot) = buf.first_mut() {
            *slot = id::CONNECTION_REQUEST;
        }
        let guid_target = buf.get_mut(1..9)?;
        guid_target.copy_from_slice(&self.client_guid.to_be_bytes());
        let time_target = buf.get_mut(9..17)?;
        time_target.copy_from_slice(&self.request_time.to_be_bytes());
        if let Some(slot) = buf.get_mut(17) {
            *slot = if self.security { 1 } else { 0 };
        }
        Some(needed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionRequestAccepted {
    pub client_address: SocketAddr,
    pub system_index: u16,
    pub system_addresses: [SocketAddr; 10],
    pub request_time: i64,
    pub server_time: i64,
}

impl ConnectionRequestAccepted {
    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        if *buf.first()? != id::CONNECTION_REQUEST_ACCEPTED {
            return None;
        }
        let rest = buf.get(1..)?;
        let (client_address, mut r) = AddressCodec::read_from(rest)?;
        let idx_slice = r.get(..2)?;
        let system_index = u16::from_be_bytes(idx_slice.try_into().ok()?);
        r = r.get(2..)?;

        let mut system_addresses = SYSTEM_ADDRESSES;
        for item in &mut system_addresses {
            let (addr, rest_addrs) = AddressCodec::read_from(r)?;
            *item = addr;
            r = rest_addrs;
        }

        let req_slice = r.get(..8)?;
        let srv_slice = r.get(8..16)?;
        let request_time = i64::from_be_bytes(req_slice.try_into().ok()?);
        let server_time = i64::from_be_bytes(srv_slice.try_into().ok()?);

        Some(Self {
            client_address,
            system_index,
            system_addresses,
            request_time,
            server_time,
        })
    }

    pub fn write_to(&self, buf: &mut [u8]) -> Option<usize> {
        let total_len = buf.len();
        if total_len == 0 {
            return None;
        }
        if let Some(slot) = buf.first_mut() {
            *slot = id::CONNECTION_REQUEST_ACCEPTED;
        }
        let rest = buf.get_mut(1..)?;
        let written_client = AddressCodec::write_to(self.client_address, rest)?;
        let mut r = rest.get_mut(written_client..)?;

        let idx_target = r.get_mut(..2)?;
        idx_target.copy_from_slice(&self.system_index.to_be_bytes());
        r = r.get_mut(2..)?;

        for addr in &self.system_addresses {
            let written = AddressCodec::write_to(*addr, r)?;
            r = r.get_mut(written..)?;
        }

        let req_target = r.get_mut(..8)?;
        req_target.copy_from_slice(&self.request_time.to_be_bytes());
        let srv_target = r.get_mut(8..16)?;
        srv_target.copy_from_slice(&self.server_time.to_be_bytes());
        let remaining_len = r.get(16..).map_or(0, |s| s.len());
        Some(total_len - remaining_len)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewIncomingConnection {
    pub server_address: SocketAddr,
    pub system_addresses: [SocketAddr; 10],
    pub ping_time: i64,
    pub pong_time: i64,
}

impl NewIncomingConnection {
    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        if *buf.first()? != id::NEW_INCOMING_CONNECTION {
            return None;
        }
        let rest = buf.get(1..)?;
        let (server_address, mut r) = AddressCodec::read_from(rest)?;
        let mut system_addresses = SYSTEM_ADDRESSES;
        for item in &mut system_addresses {
            let (addr, rest_addrs) = AddressCodec::read_from(r)?;
            *item = addr;
            r = rest_addrs;
        }
        let ping_slice = r.get(..8)?;
        let pong_slice = r.get(8..16)?;
        let ping_time = i64::from_be_bytes(ping_slice.try_into().ok()?);
        let pong_time = i64::from_be_bytes(pong_slice.try_into().ok()?);

        Some(Self {
            server_address,
            system_addresses,
            ping_time,
            pong_time,
        })
    }

    pub fn write_to(&self, buf: &mut [u8]) -> Option<usize> {
        let total_len = buf.len();
        if total_len == 0 {
            return None;
        }
        if let Some(slot) = buf.first_mut() {
            *slot = id::NEW_INCOMING_CONNECTION;
        }
        let rest = buf.get_mut(1..)?;
        let written_server = AddressCodec::write_to(self.server_address, rest)?;
        let mut r = rest.get_mut(written_server..)?;

        for addr in &self.system_addresses {
            let written = AddressCodec::write_to(*addr, r)?;
            r = r.get_mut(written..)?;
        }

        let ping_target = r.get_mut(..8)?;
        ping_target.copy_from_slice(&self.ping_time.to_be_bytes());
        let pong_target = r.get_mut(8..16)?;
        pong_target.copy_from_slice(&self.pong_time.to_be_bytes());
        let remaining_len = r.get(16..).map_or(0, |s| s.len());
        Some(total_len - remaining_len)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConnectedPing {
    pub client_time: i64,
}

impl ConnectedPing {
    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        if *buf.first()? != id::CONNECTED_PING {
            return None;
        }
        let slice = buf.get(1..9)?;
        let client_time = i64::from_be_bytes(slice.try_into().ok()?);
        Some(Self { client_time })
    }

    pub fn write_to(&self, buf: &mut [u8]) -> Option<usize> {
        if buf.len() < 9 {
            return None;
        }
        if let Some(slot) = buf.first_mut() {
            *slot = id::CONNECTED_PING;
        }
        let target = buf.get_mut(1..9)?;
        target.copy_from_slice(&self.client_time.to_be_bytes());
        Some(9)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ConnectedPong {
    pub client_time: i64,
    pub server_time: i64,
}

impl ConnectedPong {
    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        if *buf.first()? != id::CONNECTED_PONG {
            return None;
        }
        let client_slice = buf.get(1..9)?;
        let server_slice = buf.get(9..17)?;
        let client_time = i64::from_be_bytes(client_slice.try_into().ok()?);
        let server_time = i64::from_be_bytes(server_slice.try_into().ok()?);
        Some(Self {
            client_time,
            server_time,
        })
    }

    pub fn write_to(&self, buf: &mut [u8]) -> Option<usize> {
        if buf.len() < 17 {
            return None;
        }
        if let Some(slot) = buf.first_mut() {
            *slot = id::CONNECTED_PONG;
        }
        let client_target = buf.get_mut(1..9)?;
        client_target.copy_from_slice(&self.client_time.to_be_bytes());
        let server_target = buf.get_mut(9..17)?;
        server_target.copy_from_slice(&self.server_time.to_be_bytes());
        Some(17)
    }
}
