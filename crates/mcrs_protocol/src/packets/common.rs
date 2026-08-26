pub mod clientbound {
    use crate::text::Text;
    use crate::{Bounded, RawBytes};
    use derive_more::Into;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_protocol_macros::{Decode, Encode};
    use std::borrow::Cow;
    use uuid::Uuid;

    const MAX_PAYLOAD_SIZE: usize = 0x100000;

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
    pub struct CustomPayload<'a> {
        pub channel: ResourceLocation<Cow<'a, str>>,
        pub data: Bounded<Cow<'a, RawBytes<'a>>, MAX_PAYLOAD_SIZE>,
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
    use crate::{Bounded, RawBytes};
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_nbt::compound::NbtCompound;
    use mcrs_minecraft_protocol_macros::{Decode, Encode};
    use std::borrow::Cow;
    use uuid::Uuid;

    pub const MAX_PAYLOAD_SIZE: usize = 32767;

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

    #[derive(Clone, PartialEq, Debug, Encode, Decode)]
    pub struct CustomClickAction<'a> {
        pub id: ResourceLocation<Cow<'a, str>>,
        pub payload: NbtCompound,
    }
}
