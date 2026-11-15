use std::net::IpAddr;

use mcrs_minecraft_protocol::packets::login::clientbound::CustomQuery;
use mcrs_minecraft_protocol::packets::login::serverbound::ServerboundCustomQueryAnswer;
use mcrs_minecraft_protocol::profile::Property;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Decode, RawBytes, VarInt};
use mcrs_minecraft_resource::{ResourceLocation, rl};
use mcrs_minecraft_text::Text;
use sha2::{Digest, Sha256};

pub const FORWARDING_CHANNEL: ResourceLocation<'static> = rl!("velocity:player_info");
pub const REQUESTED_FORWARDING_VERSION: u8 = 4;
pub const FORWARDING_QUERY_ID: i32 = 0;

const SIGNATURE_LEN: usize = 32;
const MAX_FORWARDED_PROPERTIES: i32 = 16;

/// The `custom_query` that asks a proxy for the forwarded player.
#[must_use]
pub fn forwarding_query() -> CustomQuery<'static> {
    CustomQuery {
        message_id: VarInt(FORWARDING_QUERY_ID),
        channel: FORWARDING_CHANNEL,
        data: Bounded(RawBytes(&[REQUESTED_FORWARDING_VERSION])),
    }
}

/// The secret a proxy shares with this server, as its UTF-8 bytes, never empty.
#[derive(Clone)]
pub struct ForwardingSecret(Box<[u8]>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyForwardingSecret;

impl std::fmt::Display for EmptyForwardingSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("proxy forwarding is on, but its secret is empty")
    }
}

impl std::error::Error for EmptyForwardingSecret {}

impl std::fmt::Debug for ForwardingSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ForwardingSecret(..)")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForwardedPlayer {
    pub address: IpAddr,
    pub id: Uuid,
    pub name: String,
    pub properties: Vec<Property>,
}

/// Why a forwarding answer is refused, in the order the checks run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForwardingRefusal {
    UnexpectedAnswer,
    NoPayload,
    Unverified,
    UnsupportedVersion,
    Malformed,
}

impl ForwardingRefusal {
    #[must_use]
    pub fn reason(self) -> Text {
        match self {
            Self::UnexpectedAnswer => Text::translate(
                "multiplayer.disconnect.unexpected_query_response",
                Vec::new(),
            ),
            Self::NoPayload => Text::text("This server requires you to connect with Velocity."),
            Self::Unverified => Text::text("Unable to verify player details"),
            Self::UnsupportedVersion => Text::text("Unsupported forwarding version"),
            Self::Malformed => Text::text("Malformed player details"),
        }
    }
}

impl ForwardingSecret {
    /// The key is the secret's bytes as they are, untrimmed.
    pub fn new(secret: &str) -> Result<Self, EmptyForwardingSecret> {
        if secret.is_empty() {
            return Err(EmptyForwardingSecret);
        }
        Ok(Self(secret.as_bytes().into()))
    }

    /// Checks the answer to [`forwarding_query`] and reads the player it forwards.
    pub fn check(
        &self,
        answer: &ServerboundCustomQueryAnswer<'_>,
    ) -> Result<ForwardedPlayer, ForwardingRefusal> {
        if answer.message_id.0 != FORWARDING_QUERY_ID {
            return Err(ForwardingRefusal::UnexpectedAnswer);
        }
        let payload = answer.payload.ok_or(ForwardingRefusal::NoPayload)?.0.0;
        let (signature, mut signed) = payload
            .split_first_chunk::<SIGNATURE_LEN>()
            .ok_or(ForwardingRefusal::Unverified)?;
        if !same_bytes(signature, &hmac_sha256(&self.0, signed)) {
            return Err(ForwardingRefusal::Unverified);
        }
        let version = VarInt::decode(&mut signed).map_err(|_| ForwardingRefusal::Malformed)?;
        if !matches!(version.0, 1 | 4) {
            return Err(ForwardingRefusal::UnsupportedVersion);
        }
        let player = read_player(&mut signed).ok_or(ForwardingRefusal::Malformed)?;
        if !signed.is_empty() {
            return Err(ForwardingRefusal::Malformed);
        }
        Ok(player)
    }
}

fn read_player(r: &mut &[u8]) -> Option<ForwardedPlayer> {
    let address = Bounded::<&str, 32767>::decode(r).ok()?.0.parse().ok()?;
    let id = Uuid::decode(r).ok()?;
    let name = Bounded::<&str, 16>::decode(r).ok()?.0.to_owned();
    let count = VarInt::decode(r).ok()?.0;
    if !(0..=MAX_FORWARDED_PROPERTIES).contains(&count) {
        return None;
    }
    let properties = (0..count)
        .map(|_| {
            Some(Property {
                name: Bounded::<&str, 64>::decode(r).ok()?.0.to_owned(),
                value: Bounded::<&str, 32767>::decode(r).ok()?.0.to_owned(),
                signature: Option::<Bounded<&str, 1024>>::decode(r)
                    .ok()?
                    .map(|signature| signature.0.to_owned()),
            })
        })
        .collect::<Option<_>>()?;
    Some(ForwardedPlayer {
        address,
        id,
        name,
        properties,
    })
}

// RFC 2104 over SHA-256, whose block is 64 bytes.
fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut block = [0u8; 64];
    if key.len() > block.len() {
        block
            .iter_mut()
            .zip(Sha256::digest(key))
            .for_each(|(b, k)| *b = k);
    } else {
        block.iter_mut().zip(key).for_each(|(b, k)| *b = *k);
    }
    let pad = |byte: u8| block.map(|k| k ^ byte);
    let inner = Sha256::new()
        .chain_update(pad(0x36))
        .chain_update(data)
        .finalize();
    Sha256::new()
        .chain_update(pad(0x5c))
        .chain_update(inner)
        .finalize()
        .into()
}

// Reads every byte, so the time does not tell where the first difference is.
fn same_bytes(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let difference = a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y));
    std::hint::black_box(difference) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_protocol::Encode;

    const SIGNED_V4: &str = "040b3230332e302e3131332e37001122334455667788\
        99aabbccddeeff0553746576650108746578747572657304653330 3d00";
    const SIGNATURE_V4: &str = "0be13f34759c06934e5a85851568731bf3aa5267c3f7511e0b33d16ea4b81459";
    const SIGNATURE_V1: &str = "cfdf311b41db160b1ca01c1816a055ec6cc71769186b966f94ff5d56447060cb";

    fn hex(text: &str) -> Vec<u8> {
        let digits: Vec<u8> = text.bytes().filter(u8::is_ascii_hexdigit).collect();
        digits
            .chunks(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn secret() -> ForwardingSecret {
        ForwardingSecret::new("example-secret").unwrap()
    }

    fn signed(key: &str, data: &[u8]) -> Vec<u8> {
        let mut payload = hmac_sha256(key.as_bytes(), data).to_vec();
        payload.extend_from_slice(data);
        payload
    }

    fn check(
        message_id: i32,
        payload: Option<&[u8]>,
    ) -> Result<ForwardedPlayer, ForwardingRefusal> {
        secret().check(&ServerboundCustomQueryAnswer {
            message_id: VarInt(message_id),
            payload: payload.map(|bytes| Bounded(RawBytes(bytes))),
        })
    }

    fn with_version(version: u8) -> Vec<u8> {
        let mut data = hex(SIGNED_V4);
        data[0] = version;
        data
    }

    #[test]
    fn the_signatures_of_the_test_vectors() {
        let v4 = hex(SIGNED_V4);
        assert_eq!(v4.len(), 51);
        assert_eq!(
            hmac_sha256(b"example-secret", &v4).to_vec(),
            hex(SIGNATURE_V4)
        );
        assert_eq!(
            hmac_sha256(b"example-secret", &with_version(1)).to_vec(),
            hex(SIGNATURE_V1)
        );
    }

    #[test]
    fn a_signed_payload_of_version_one_or_four_forwards_its_player() {
        let expected = ForwardedPlayer {
            address: "203.0.113.7".parse().unwrap(),
            id: "00112233-4455-6677-8899-aabbccddeeff".parse().unwrap(),
            name: "Steve".to_owned(),
            properties: vec![Property {
                name: "textures".to_owned(),
                value: "e30=".to_owned(),
                signature: None,
            }],
        };
        for (signature, data) in [
            (SIGNATURE_V4, with_version(4)),
            (SIGNATURE_V1, with_version(1)),
        ] {
            let mut payload = hex(signature);
            payload.extend_from_slice(&data);
            assert_eq!(payload.len(), 83);
            assert_eq!(
                check(FORWARDING_QUERY_ID, Some(&payload)),
                Ok(expected.clone())
            );
        }
    }

    #[test]
    fn each_failed_check_gives_its_refusal() {
        let mut flipped = signed("example-secret", &hex(SIGNED_V4));
        if let Some(byte) = flipped.last_mut() {
            *byte ^= 1;
        }
        let mut trailing = hex(SIGNED_V4);
        trailing.push(0);
        let mut bad_address = Vec::new();
        VarInt(4).encode(&mut bad_address).unwrap();
        "example.com".encode(&mut bad_address).unwrap();
        bad_address.extend_from_slice(&hex(SIGNED_V4)[13..]);
        let mut too_many_properties = hex(SIGNED_V4)[..35].to_vec();
        too_many_properties.push(17);
        let cases: [(i32, Option<Vec<u8>>, ForwardingRefusal); 10] = [
            (
                1,
                Some(signed("example-secret", &hex(SIGNED_V4))),
                ForwardingRefusal::UnexpectedAnswer,
            ),
            (FORWARDING_QUERY_ID, None, ForwardingRefusal::NoPayload),
            (
                FORWARDING_QUERY_ID,
                Some(vec![0; 31]),
                ForwardingRefusal::Unverified,
            ),
            (
                FORWARDING_QUERY_ID,
                Some(flipped),
                ForwardingRefusal::Unverified,
            ),
            (
                FORWARDING_QUERY_ID,
                Some(signed("example-secret\n", &hex(SIGNED_V4))),
                ForwardingRefusal::Unverified,
            ),
            (
                FORWARDING_QUERY_ID,
                Some(signed("example-secret", &with_version(5))),
                ForwardingRefusal::UnsupportedVersion,
            ),
            (
                FORWARDING_QUERY_ID,
                Some(signed("example-secret", &with_version(3))),
                ForwardingRefusal::UnsupportedVersion,
            ),
            (
                FORWARDING_QUERY_ID,
                Some(signed("example-secret", &trailing)),
                ForwardingRefusal::Malformed,
            ),
            (
                FORWARDING_QUERY_ID,
                Some(signed("example-secret", &bad_address)),
                ForwardingRefusal::Malformed,
            ),
            (
                FORWARDING_QUERY_ID,
                Some(signed("example-secret", &too_many_properties)),
                ForwardingRefusal::Malformed,
            ),
        ];
        for (message_id, payload, refusal) in cases {
            assert_eq!(
                check(message_id, payload.as_deref()),
                Err(refusal),
                "{refusal:?}"
            );
        }
    }

    #[test]
    fn an_empty_secret_is_a_configuration_error() {
        assert_eq!(ForwardingSecret::new("").err(), Some(EmptyForwardingSecret));
    }
}
