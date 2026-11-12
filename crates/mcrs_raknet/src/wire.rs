extern crate alloc;

use core::fmt;
use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uint24(u32);

impl Uint24 {
    pub const MAX: Self = Self(0x00FF_FFFF);

    #[inline]
    #[must_use]
    pub const fn new(val: u32) -> Self {
        Self(val & 0x00FF_FFFF)
    }

    #[inline]
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    #[inline]
    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<(Self, &[u8])> {
        let b0 = *buf.first()?;
        let b1 = *buf.get(1)?;
        let b2 = *buf.get(2)?;
        let val = (b0 as u32) | ((b1 as u32) << 8) | ((b2 as u32) << 16);
        let rest = buf.get(3..)?;
        Some((Self(val), rest))
    }

    #[inline]
    pub fn write_to(self, buf: &mut [u8]) -> Option<&mut [u8]> {
        if buf.len() < 3 {
            return None;
        }
        let (head, rest) = buf.split_at_mut(3);
        if let Some(slot) = head.first_mut() {
            *slot = self.0 as u8;
        }
        if let Some(slot) = head.get_mut(1) {
            *slot = (self.0 >> 8) as u8;
        }
        if let Some(slot) = head.get_mut(2) {
            *slot = (self.0 >> 16) as u8;
        }
        Some(rest)
    }

    #[inline]
    #[must_use]
    pub const fn wrapping_add(self, rhs: u32) -> Self {
        Self(self.0.wrapping_add(rhs) & 0x00FF_FFFF)
    }

    #[inline]
    #[must_use]
    pub const fn wrapping_sub(self, other: Self) -> u32 {
        self.0.wrapping_sub(other.0) & 0x00FF_FFFF
    }

    #[inline]
    #[must_use]
    pub fn is_newer_than(self, other: Self) -> bool {
        let diff = self.wrapping_sub(other);
        diff > 0 && diff < (1 << 23)
    }

    #[inline]
    #[must_use]
    pub fn is_in_range_wrapped(val: Self, start: Self, end: Self) -> bool {
        if start.0 <= end.0 {
            val.0 >= start.0 && val.0 <= end.0
        } else {
            val.0 >= start.0 || val.0 <= end.0
        }
    }
}

impl fmt::Debug for Uint24 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for Uint24 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u32> for Uint24 {
    #[inline]
    fn from(val: u32) -> Self {
        Self::new(val)
    }
}

impl From<Uint24> for u32 {
    #[inline]
    fn from(val: Uint24) -> Self {
        val.0
    }
}

pub struct AddressCodec;

impl AddressCodec {
    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<(SocketAddr, &[u8])> {
        let version = *buf.first()?;
        let rest = buf.get(1..)?;

        match version {
            4 => {
                let ip_bytes = rest.get(..4)?;
                let octets: [u8; 4] = [
                    *ip_bytes.first()?,
                    *ip_bytes.get(1)?,
                    *ip_bytes.get(2)?,
                    *ip_bytes.get(3)?,
                ];
                let port_bytes = rest.get(4..6)?;
                let port = u16::from_be_bytes([*port_bytes.first()?, *port_bytes.get(1)?]);
                let remaining = rest.get(6..)?;
                Some((
                    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from(octets), port)),
                    remaining,
                ))
            }
            6 => {
                let family_bytes = rest.get(..2)?;
                let _family = u16::from_le_bytes([*family_bytes.first()?, *family_bytes.get(1)?]);
                let port_bytes = rest.get(2..4)?;
                let port = u16::from_be_bytes([*port_bytes.first()?, *port_bytes.get(1)?]);
                let _flow_info = rest.get(4..8)?;
                let ip_slice = rest.get(8..24)?;
                let mut octets = [0u8; 16];
                for (dst, src) in octets.iter_mut().zip(ip_slice.iter()) {
                    *dst = *src;
                }
                let _scope_id = rest.get(24..28)?;
                let remaining = rest.get(28..)?;
                Some((
                    SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from(octets), port, 0, 0)),
                    remaining,
                ))
            }
            _ => None,
        }
    }

    pub fn write_to(addr: SocketAddr, buf: &mut [u8]) -> Option<usize> {
        match addr {
            SocketAddr::V4(v4) => {
                if buf.len() < 7 {
                    return None;
                }
                if let Some(slot) = buf.first_mut() {
                    *slot = 4;
                }
                let octets = v4.ip().octets();
                if let Some(dst) = buf.get_mut(1..5) {
                    dst.copy_from_slice(&octets);
                }
                let port_bytes = v4.port().to_be_bytes();
                if let Some(dst) = buf.get_mut(5..7) {
                    dst.copy_from_slice(&port_bytes);
                }
                Some(7)
            }
            SocketAddr::V6(v6) => {
                if buf.len() < 29 {
                    return None;
                }
                if let Some(slot) = buf.first_mut() {
                    *slot = 6;
                }
                if let Some(dst) = buf.get_mut(1..3) {
                    dst.copy_from_slice(&23u16.to_le_bytes());
                }
                if let Some(dst) = buf.get_mut(3..5) {
                    dst.copy_from_slice(&v6.port().to_be_bytes());
                }
                if let Some(dst) = buf.get_mut(5..9) {
                    dst.copy_from_slice(&[0u8; 4]);
                }
                let octets = v6.ip().octets();
                if let Some(dst) = buf.get_mut(9..25) {
                    dst.copy_from_slice(&octets);
                }
                if let Some(dst) = buf.get_mut(25..29) {
                    dst.copy_from_slice(&[0u8; 4]);
                }
                Some(29)
            }
        }
    }
}
