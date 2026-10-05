use crate::{Decode, Encode};
use mcrs_minecraft_core::{ResourceLocation, rl};

pub const BRAND_CHANNEL: ResourceLocation<&'static str> = rl!("minecraft:brand");
pub const MOD_LIST_CHANNEL: ResourceLocation<&'static str> = rl!("minecraft:mod_list");

#[derive(Copy, Clone, PartialEq, Eq, Debug, Encode, Decode)]
pub struct Brand<'a> {
    pub brand: &'a str,
}

pub mod clientbound {
    use super::{BRAND_CHANNEL, Brand};
    use crate::text::Text;
    use crate::{Bounded, Decode, Encode, RawBytes};
    use derive_more::Into;
    use mcrs_minecraft_core::ResourceLocation;
    use std::borrow::Cow;
    use std::io::Write;
    use uuid::Uuid;

    pub const MAX_PAYLOAD_SIZE: usize = 0x100000;

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
    pub struct CustomPayload<'a> {
        pub channel: ResourceLocation<Cow<'a, str>>,
        pub data: Bounded<Cow<'a, RawBytes<'a>>, MAX_PAYLOAD_SIZE>,
    }

    #[derive(Clone, PartialEq, Eq, Debug)]
    pub enum Payload<'a> {
        Brand(Brand<'a>),
        Raw(CustomPayload<'a>),
    }

    impl Encode for Payload<'_> {
        fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
            match self {
                Payload::Brand(brand) => {
                    BRAND_CHANNEL.encode(&mut w)?;
                    brand.encode(w)
                }
                Payload::Raw(raw) => {
                    anyhow::ensure!(
                        raw.channel != BRAND_CHANNEL,
                        "the channel {} is a typed payload",
                        raw.channel
                    );
                    raw.encode(w)
                }
            }
        }
    }

    impl<'a> Decode<'a> for Payload<'a> {
        fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
            let channel = ResourceLocation::decode(r)?;
            if channel == BRAND_CHANNEL {
                return Ok(Payload::Brand(Brand::decode(r)?));
            }
            Ok(Payload::Raw(CustomPayload {
                channel,
                data: Decode::decode(r)?,
            }))
        }
    }

    #[derive(Clone, Debug, Encode, Decode, Into)]
    pub struct Disconnect<'a> {
        pub reason: Cow<'a, Text>,
    }

    #[derive(Copy, Clone, PartialEq, Eq, Debug, Encode, Decode)]
    pub struct KeepAlive {
        pub payload: i64,
    }

    #[derive(Copy, Clone, PartialEq, Eq, Debug, Encode, Decode)]
    pub struct Ping {
        pub payload: i32,
    }

    #[derive(Copy, Clone, PartialEq, Eq, Debug, Encode, Decode)]
    pub struct ResourcePackPop {
        pub id: Option<Uuid>,
    }

    #[derive(Clone, PartialEq, Debug, Encode, Decode)]
    pub struct ResourcePackPush<'a> {
        pub id: Uuid,
        pub url: &'a str,
        pub hash: &'a str,
        pub required: bool,
        pub prompt: Option<Cow<'a, Text>>,
    }
}

pub mod serverbound {
    use super::{BRAND_CHANNEL, Brand, MOD_LIST_CHANNEL};
    use crate::{Bounded, Decode, Encode, RawBytes, VarInt};
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_nbt::tag::NbtTag;
    use std::borrow::Cow;
    use std::io::Write;
    use uuid::Uuid;

    pub const MAX_PAYLOAD_SIZE: usize = 32767;
    pub const MAX_MOD_ENTRIES: usize = 8192;
    pub const MAX_PROPERTIES: usize = 32;
    pub const MAX_PROPERTY_CHARS: usize = 128;

    #[derive(Copy, Clone, PartialEq, Eq, Debug, Encode, Decode)]
    pub struct ClientInformation<'a> {
        pub locale: &'a str,
        pub view_distance: u8,
        pub chat_mode: crate::setting::ChatMode,
        pub chat_colors: bool,
        pub displayed_skin_parts: crate::setting::DisplayedSkinParts,
        pub main_arm: crate::setting::MainArm,
        pub enable_text_filtering: bool,
        pub allow_server_listings: bool,
        pub particle_status: crate::setting::ParticleStatus,
    }

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
    pub struct CustomPayload<'a> {
        pub channel: ResourceLocation<Cow<'a, str>>,
        pub data: Bounded<RawBytes<'a>, MAX_PAYLOAD_SIZE>,
    }

    #[derive(Clone, PartialEq, Eq, Debug, Default)]
    pub struct PropertyMap<'a>(pub Vec<(ResourceLocation<Cow<'a, str>>, &'a str)>);

    #[derive(Clone, PartialEq, Eq, Debug, Default)]
    pub struct ModList<'a>(pub Vec<(ResourceLocation<Cow<'a, str>>, PropertyMap<'a>)>);

    fn write_count(w: impl Write, len: usize, max: usize, what: &str) -> anyhow::Result<()> {
        anyhow::ensure!(len <= max, "{what} of {len} entries exceeds {max}");
        VarInt(len as i32).encode(w)
    }

    fn read_count(r: &mut &[u8], max: usize, what: &str) -> anyhow::Result<usize> {
        let len = VarInt::decode(r)?.0;
        anyhow::ensure!(len >= 0, "{what} has a negative length");
        anyhow::ensure!(len as usize <= max, "{what} of {len} entries exceeds {max}");
        Ok(len as usize)
    }

    impl Encode for PropertyMap<'_> {
        fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
            write_count(&mut w, self.0.len(), MAX_PROPERTIES, "a property map")?;
            for (key, value) in &self.0 {
                key.encode(&mut w)?;
                Bounded::<_, MAX_PROPERTY_CHARS>(*value).encode(&mut w)?;
            }
            Ok(())
        }
    }

    impl<'a> Decode<'a> for PropertyMap<'a> {
        fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
            let len = read_count(r, MAX_PROPERTIES, "a property map")?;
            let mut pairs = Vec::with_capacity(len.min(r.len()));
            for _ in 0..len {
                let key = ResourceLocation::decode(r)?;
                let Bounded(value) = Bounded::<&str, MAX_PROPERTY_CHARS>::decode(r)?;
                pairs.push((key, value));
            }
            Ok(Self(pairs))
        }
    }

    impl Encode for ModList<'_> {
        fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
            write_count(&mut w, self.0.len(), MAX_MOD_ENTRIES, "a mod list")?;
            for (key, properties) in &self.0 {
                key.encode(&mut w)?;
                properties.encode(&mut w)?;
            }
            Ok(())
        }
    }

    impl<'a> Decode<'a> for ModList<'a> {
        fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
            let len = read_count(r, MAX_MOD_ENTRIES, "a mod list")?;
            let mut entries = Vec::with_capacity(len.min(r.len()));
            for _ in 0..len {
                entries.push((ResourceLocation::decode(r)?, PropertyMap::decode(r)?));
            }
            Ok(Self(entries))
        }
    }

    #[derive(Clone, PartialEq, Eq, Debug)]
    pub enum Payload<'a> {
        Brand(Brand<'a>),
        ModList(ModList<'a>),
        Raw(CustomPayload<'a>),
    }

    impl Encode for Payload<'_> {
        fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
            match self {
                Payload::Brand(brand) => {
                    BRAND_CHANNEL.encode(&mut w)?;
                    brand.encode(w)
                }
                Payload::ModList(mods) => {
                    MOD_LIST_CHANNEL.encode(&mut w)?;
                    mods.encode(w)
                }
                Payload::Raw(raw) => {
                    anyhow::ensure!(
                        raw.channel != BRAND_CHANNEL && raw.channel != MOD_LIST_CHANNEL,
                        "the channel {} is a typed payload",
                        raw.channel
                    );
                    raw.encode(w)
                }
            }
        }
    }

    impl<'a> Decode<'a> for Payload<'a> {
        fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
            let channel = ResourceLocation::decode(r)?;
            if channel == BRAND_CHANNEL {
                return Ok(Payload::Brand(Brand::decode(r)?));
            }
            if channel == MOD_LIST_CHANNEL {
                return Ok(Payload::ModList(ModList::decode(r)?));
            }
            Ok(Payload::Raw(CustomPayload {
                channel,
                data: Decode::decode(r)?,
            }))
        }
    }

    #[derive(Copy, Clone, PartialEq, Eq, Debug, Encode, Decode)]
    pub struct KeepAlive {
        pub payload: i64,
    }

    #[derive(Copy, Clone, PartialEq, Eq, Debug, Encode, Decode)]
    pub struct Pong {
        pub payload: i32,
    }

    #[derive(Copy, Clone, PartialEq, Eq, Debug, Encode, Decode)]
    pub struct ResourcePack {
        pub id: Uuid,
        pub status: crate::resource_pack::Status,
    }

    pub const MAX_CLICK_PAYLOAD: usize = 65536;

    #[derive(Clone, PartialEq, Debug)]
    pub struct CustomClickAction<'a> {
        pub id: ResourceLocation<Cow<'a, str>>,
        pub payload: Option<NbtTag>,
    }

    impl Encode for CustomClickAction<'_> {
        fn encode(&self, mut w: impl std::io::Write) -> anyhow::Result<()> {
            self.id.encode(&mut w)?;
            let mut body = Vec::new();
            match &self.payload {
                None => body.push(NbtTag::End.get_type_id()),
                Some(NbtTag::End) => anyhow::bail!("a click action payload cannot be an end tag"),
                Some(tag) => tag.encode(&mut body)?,
            }
            anyhow::ensure!(
                body.len() <= MAX_CLICK_PAYLOAD,
                "a click action payload of {} bytes exceeds {MAX_CLICK_PAYLOAD}",
                body.len()
            );
            body.as_slice().encode(w)
        }
    }

    impl<'a> Decode<'a> for CustomClickAction<'a> {
        fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
            let id = ResourceLocation::decode(r)?;
            let Bounded(mut body) = Bounded::<&[u8], MAX_CLICK_PAYLOAD>::decode(r)?;
            let payload = match NbtTag::decode(&mut body)? {
                NbtTag::End => None,
                tag => Some(tag),
            };
            anyhow::ensure!(
                body.is_empty(),
                "{} bytes follow a click action payload",
                body.len()
            );
            Ok(Self { id, payload })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::serverbound::{
        MAX_MOD_ENTRIES, MAX_PROPERTIES, MAX_PROPERTY_CHARS, ModList, PropertyMap,
    };
    use crate::{Decode, Encode, VarInt};
    use mcrs_minecraft_core::ResourceLocation;
    use std::borrow::Cow;

    fn key(path: &str) -> ResourceLocation<Cow<'_, str>> {
        ResourceLocation::read_cow(path).unwrap()
    }

    fn count(n: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        VarInt(n as i32).encode(&mut bytes).unwrap();
        bytes
    }

    fn encoded(value: &impl Encode) -> anyhow::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        value.encode(&mut bytes)?;
        Ok(bytes)
    }

    fn property_map_bytes(properties: usize, value: &str) -> Vec<u8> {
        let mut bytes = count(properties);
        for _ in 0..properties {
            "mcrs:commit".encode(&mut bytes).unwrap();
            bytes.extend(count(value.len()));
            bytes.extend_from_slice(value.as_bytes());
        }
        bytes
    }

    fn mod_list_bytes(entries: usize) -> Vec<u8> {
        let mut bytes = count(entries);
        for _ in 0..entries {
            "github.com:andreypfau/mcrs".encode(&mut bytes).unwrap();
            bytes.extend(count(0));
        }
        bytes
    }

    #[test]
    fn a_mod_list_holds_at_most_its_entry_limit() {
        let mut r = &mod_list_bytes(MAX_MOD_ENTRIES)[..];
        assert_eq!(ModList::decode(&mut r).unwrap().0.len(), MAX_MOD_ENTRIES);
        assert!(r.is_empty());
        assert!(ModList::decode(&mut &mod_list_bytes(MAX_MOD_ENTRIES + 1)[..]).is_err());

        let at_limit = ModList(vec![(key("a:b"), PropertyMap::default()); MAX_MOD_ENTRIES]);
        assert!(encoded(&at_limit).is_ok());
        let past = ModList(vec![
            (key("a:b"), PropertyMap::default());
            MAX_MOD_ENTRIES + 1
        ]);
        assert!(encoded(&past).is_err());
    }

    #[test]
    fn a_property_map_holds_at_most_its_property_limit() {
        let mut r = &property_map_bytes(MAX_PROPERTIES, "x")[..];
        assert_eq!(PropertyMap::decode(&mut r).unwrap().0.len(), MAX_PROPERTIES);
        assert!(r.is_empty());
        assert!(
            PropertyMap::decode(&mut &property_map_bytes(MAX_PROPERTIES + 1, "x")[..]).is_err()
        );

        let at_limit = PropertyMap(vec![(key("mcrs:commit"), "x"); MAX_PROPERTIES]);
        assert_eq!(
            encoded(&at_limit).unwrap(),
            property_map_bytes(MAX_PROPERTIES, "x")
        );
        let past = PropertyMap(vec![(key("mcrs:commit"), "x"); MAX_PROPERTIES + 1]);
        assert!(encoded(&past).is_err());
    }

    #[test]
    fn a_property_value_holds_at_most_its_character_limit() {
        let at_limit = "a".repeat(MAX_PROPERTY_CHARS);
        let past = "a".repeat(MAX_PROPERTY_CHARS + 1);
        let bytes = property_map_bytes(1, &at_limit);
        let decoded = PropertyMap::decode(&mut &bytes[..]).unwrap();
        assert_eq!(decoded.0[0].1, at_limit);
        assert_eq!(encoded(&decoded).unwrap(), bytes);
        assert!(PropertyMap::decode(&mut &property_map_bytes(1, &past)[..]).is_err());
        assert!(encoded(&PropertyMap(vec![(key("mcrs:commit"), &past)])).is_err());

        let wide_at_limit = "\u{1F600}".repeat(MAX_PROPERTY_CHARS / 2);
        let wide_past = "\u{1F600}".repeat(MAX_PROPERTY_CHARS / 2 + 1);
        assert!(PropertyMap::decode(&mut &property_map_bytes(1, &wide_at_limit)[..]).is_ok());
        assert!(PropertyMap::decode(&mut &property_map_bytes(1, &wide_past)[..]).is_err());
    }
}
