use std::net::IpAddr;

use hmac::{Hmac, KeyInit, Mac};
use mcrs_minecraft_protocol::packets::login::clientbound::CustomQuery;
use mcrs_minecraft_protocol::packets::login::serverbound::ServerboundCustomQueryAnswer;
use mcrs_minecraft_protocol::profile::Property;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Decode, RawBytes, VarInt};
use mcrs_minecraft_resource::{ResourceLocation, rl};
use mcrs_minecraft_text::Text;
use sha2::Sha256;

pub const FORWARDING_CHANNEL: ResourceLocation<'static> = rl!("velocity:player_info");
const REQUESTED_FORWARDING_VERSION: u8 = 4;
const FORWARDING_QUERY_ID: i32 = 0;

const SIGNATURE_LEN: usize = 32;
const MAX_FORWARDED_PROPERTIES: i32 = 16;

#[must_use]
pub fn forwarding_query() -> CustomQuery<'static> {
    CustomQuery {
        message_id: VarInt(FORWARDING_QUERY_ID),
        channel: FORWARDING_CHANNEL,
        data: Bounded(RawBytes(&[REQUESTED_FORWARDING_VERSION])),
    }
}

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
    pub fn new(secret: &str) -> Result<Self, EmptyForwardingSecret> {
        if secret.is_empty() {
            return Err(EmptyForwardingSecret);
        }
        Ok(Self(secret.as_bytes().into()))
    }

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
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&self.0).map_err(|_| ForwardingRefusal::Unverified)?;
        mac.update(signed);
        mac.verify_slice(signature)
            .map_err(|_| ForwardingRefusal::Unverified)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_protocol::Encode;

    const KEY: &str = "example-secret";
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

    fn signature(key: &str, data: &[u8]) -> Vec<u8> {
        let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).unwrap();
        mac.update(data);
        mac.finalize().into_bytes().to_vec()
    }

    fn signed(key: &str, data: &[u8]) -> Vec<u8> {
        let mut payload = signature(key, data);
        payload.extend_from_slice(data);
        payload
    }

    fn check(
        message_id: i32,
        payload: Option<&[u8]>,
    ) -> Result<ForwardedPlayer, ForwardingRefusal> {
        ForwardingSecret::new(KEY)
            .unwrap()
            .check(&ServerboundCustomQueryAnswer {
                message_id: VarInt(message_id),
                payload: payload.map(|bytes| Bounded(RawBytes(bytes))),
            })
    }

    fn with_version(version: u8) -> Vec<u8> {
        let mut data = hex(SIGNED_V4);
        data[0] = version;
        data
    }

    fn string(text: &str, out: &mut Vec<u8>) {
        Bounded::<&str, 1_000_000>(text).encode(out).unwrap();
    }

    fn player(name: &str, count: usize, field: [usize; 3]) -> Vec<u8> {
        let mut data = vec![4];
        string("203.0.113.7", &mut data);
        Uuid::nil().encode(&mut data).unwrap();
        string(name, &mut data);
        VarInt(i32::try_from(count).unwrap())
            .encode(&mut data)
            .unwrap();
        for _ in 0..count {
            string(&"n".repeat(field[0]), &mut data);
            string(&"v".repeat(field[1]), &mut data);
            true.encode(&mut data).unwrap();
            string(&"s".repeat(field[2]), &mut data);
        }
        data
    }

    #[test]
    fn the_signatures_of_the_test_vectors() {
        let v4 = hex(SIGNED_V4);
        assert_eq!(v4.len(), 51);
        assert_eq!(signature(KEY, &v4), hex(SIGNATURE_V4));
        assert_eq!(signature(KEY, &with_version(1)), hex(SIGNATURE_V1));
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
    fn a_profile_at_every_limit_is_forwarded() {
        let name = "a".repeat(16);
        let payload = signed(KEY, &player(&name, 16, [64, 32767, 1024]));
        let player = check(FORWARDING_QUERY_ID, Some(&payload)).unwrap();
        assert_eq!(player.name, name);
        assert_eq!(player.properties.len(), 16);
        let property = &player.properties[15];
        assert_eq!(
            (
                property.name.len(),
                property.value.len(),
                property.signature.as_deref().map(str::len)
            ),
            (64, 32767, Some(1024))
        );
    }

    #[test]
    fn each_failed_check_gives_its_refusal_and_text() {
        let mut flipped = signed(KEY, &hex(SIGNED_V4));
        if let Some(byte) = flipped.last_mut() {
            *byte ^= 1;
        }
        let mut trailing = hex(SIGNED_V4);
        trailing.push(0);
        let mut bad_address = Vec::new();
        VarInt(4).encode(&mut bad_address).unwrap();
        string("example.com", &mut bad_address);
        bad_address.extend_from_slice(&hex(SIGNED_V4)[13..]);
        let unexpected = Text::translate(
            "multiplayer.disconnect.unexpected_query_response",
            Vec::new(),
        );
        let velocity = Text::text("This server requires you to connect with Velocity.");
        let unverified = Text::text("Unable to verify player details");
        let unsupported = Text::text("Unsupported forwarding version");
        let malformed = Text::text("Malformed player details");
        let id = FORWARDING_QUERY_ID;
        let signed_data = |data: &[u8]| Some(signed(KEY, data));
        let cases = [
            (1, signed_data(&hex(SIGNED_V4)), unexpected),
            (id, None, velocity),
            (id, Some(vec![0; 31]), unverified.clone()),
            (id, Some(flipped), unverified.clone()),
            (
                id,
                Some(signed("example-secret\n", &hex(SIGNED_V4))),
                unverified,
            ),
            (id, signed_data(&with_version(5)), unsupported.clone()),
            (id, signed_data(&with_version(3)), unsupported),
            (id, signed_data(&trailing), malformed.clone()),
            (id, signed_data(&bad_address), malformed.clone()),
            (
                id,
                signed_data(&player(&"a".repeat(17), 0, [0; 3])),
                malformed.clone(),
            ),
            (
                id,
                signed_data(&player("a", 17, [1, 1, 1])),
                malformed.clone(),
            ),
            (
                id,
                signed_data(&player("a", 1, [65, 1, 1])),
                malformed.clone(),
            ),
            (
                id,
                signed_data(&player("a", 1, [1, 32768, 1])),
                malformed.clone(),
            ),
            (id, signed_data(&player("a", 1, [1, 1, 1025])), malformed),
        ];
        for (case, (message_id, payload, text)) in cases.into_iter().enumerate() {
            let refusal = check(message_id, payload.as_deref()).unwrap_err();
            assert_eq!(refusal.reason(), text, "case {case}: {refusal:?}");
        }
    }

    #[test]
    fn an_empty_secret_is_a_configuration_error() {
        assert_eq!(ForwardingSecret::new("").err(), Some(EmptyForwardingSecret));
    }
}
