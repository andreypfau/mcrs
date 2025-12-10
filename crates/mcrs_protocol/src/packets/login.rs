pub mod clientbound {
    use crate::packets::cookie::clientbound::CookieRequest;
    use crate::profile::GameProfile;
    use crate::{Bounded, RawBytes, VarInt};
    use derive_more::{From, Into};
    use mcrs_protocol_macros::{Decode, Encode, Packet};
    use std::borrow::Cow;
    use valence_ident::Ident;

    #[derive(Copy, Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x00, state=Login)]
    pub struct ClientboundLoginDisconnect<'a> {
        pub reason: Bounded<&'a str, 32767>,
    }

    #[derive(Copy, Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x01, state=Login)]
    pub struct ClientboundHello<'a> {
        pub server_id: Bounded<&'a str, 20>,
        pub public_key: &'a [u8],
        pub verify_token: &'a [u8],
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x02, state=Login)]
    pub struct ClientboundLoginFinished<'a> {
        pub profile: GameProfile<'a>,
    }

    #[derive(Copy, Clone, Debug, Encode, Decode, Into)]
    pub struct LoginCompression {
        pub threshold: VarInt,
    }

    #[derive(Clone, Debug, Encode, Decode, Into)]
    pub struct CustomQuery<'a> {
        pub message_id: VarInt,
        pub channel: Ident<Cow<'a, str>>,
        pub data: Bounded<RawBytes<'a>, 1048576>,
    }

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub enum Packet<'a> {
        LoginDisconnect(ClientboundLoginDisconnect<'a>),
        Hello(ClientboundHello<'a>),
        LoginFinished(ClientboundLoginFinished<'a>),
        LoginCompression(LoginCompression),
        CustomQuery(CustomQuery<'a>),
        CookieRequest(CookieRequest<'a>),
    }
}

pub mod serverbound {
    use crate::packets::cookie::serverbound::CookieResponse;
    use crate::{Bounded, RawBytes, VarInt};
    use derive_more::{From, Into};
    use mcrs_protocol_macros::{Decode, Encode, Packet};
    use uuid::Uuid;

    #[derive(Copy, Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x00, state=Login)]
    pub struct ServerboundHello<'a> {
        pub username: Bounded<&'a str, 16>,
        pub profile_id: Uuid,
    }

    #[derive(Copy, Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x01, state=Login)]
    pub struct ServerboundKey<'a> {
        pub shared_secret: &'a [u8],
        pub verify_token: &'a [u8],
    }

    #[derive(Copy, Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x02, state=Login)]
    pub struct ServerboundCustomQueryAnswer<'a> {
        pub message_id: VarInt,
        pub payload: Option<Bounded<RawBytes<'a>, 1048576>>,
    }

    #[derive(Copy, Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x03, state=Login)]
    pub struct ServerboundLoginAcknowledged;

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x04, state=Login)]
    pub struct ServerboundCookieResponse<'a>(CookieResponse<'a>);

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub enum ServerboundPacket<'a> {
        Hello(ServerboundHello<'a>),
        Key(ServerboundKey<'a>),
        CustomQueryAnswer(ServerboundCustomQueryAnswer<'a>),
        LoginAcknowledged(ServerboundLoginAcknowledged),
        CookieResponse(ServerboundCookieResponse<'a>),
    }
}
