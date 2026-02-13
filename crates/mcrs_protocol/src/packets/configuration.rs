pub use self::clientbound::ClientboundFinishConfiguration;
pub use self::clientbound::ClientboundKeepAlive;
pub use self::clientbound::ClientboundRegistryData;
pub use self::clientbound::ClientboundShowDialog;

pub mod clientbound {
    use crate::packets::common::clientbound::{CustomPayload, Disconnect, KeepAlive, Ping};
    use crate::packets::cookie::clientbound::CookieRequest;
    use derive_more::From;
    use mcrs_nbt::compound::NbtCompound;
    use mcrs_protocol_macros::{Decode, Encode, Packet};
    use std::borrow::Cow;
    use valence_ident::Ident;

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x03, state=Configuration)]
    pub struct ClientboundFinishConfiguration;

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x04, state=Configuration)]
    pub struct ClientboundKeepAlive(pub KeepAlive);

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x07, state=Configuration)]
    pub struct ClientboundRegistryData<'a> {
        pub registry: Ident<Cow<'a, str>>,
        pub entries: Vec<crate::registry::Entry<'a>>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x0E, state=Configuration)]
    pub struct ClientboundSelectKnownPacks<'a> {
        pub known_packs: Vec<crate::resource_pack::KnownPack<'a>>,
    }

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x12, state=Configuration)]
    pub struct ClientboundShowDialog {
        pub dialog: NbtCompound,
    }

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub enum Packet<'a> {
        CookieRequest(CookieRequest<'a>),
        CustomPayload(CustomPayload<'a>),
        Disconnect(Disconnect<'a>),
        FinishConfiguration,
        KeepAlive(KeepAlive),
        Ping(Ping),
        ResetChat,
        RegistryData(ClientboundRegistryData<'a>),
    }
}

pub mod serverbound {
    use crate::PacketSide;
    use crate::packets::common::serverbound::{
        ClientInformation, CustomClickAction, KeepAlive, Pong, ResourcePack,
    };
    use crate::resource_pack::KnownPack;
    use derive_more::From;
    use mcrs_protocol_macros::{Decode, Encode, Packet};

    #[derive(Clone, Debug, Encode, Decode, From, Packet)]
    #[packet(id=0x00, side=PacketSide::Serverbound, state=Configuration)]
    pub struct ServerboundClientInformation<'a>(pub ClientInformation<'a>);

    #[derive(Clone, Debug, Encode, Decode, From, Packet)]
    #[packet(id=0x01, state=Configuration)]
    pub struct ServerboundCookieResponse<'a>(
        crate::packets::cookie::serverbound::CookieResponse<'a>,
    );

    #[derive(Clone, Debug, Encode, Decode, From, Packet)]
    #[packet(id=0x02, state=Configuration)]
    pub struct ServerboundCustomPayload<'a>(crate::packets::common::serverbound::CustomPayload<'a>);

    #[derive(Clone, Debug, Encode, Decode, From, Packet)]
    #[packet(id=0x03, state=Configuration)]
    pub struct ServerboundFinishConfiguration;

    #[derive(Clone, Debug, Encode, Decode, From, Packet)]
    #[packet(id=0x04, state=Configuration)]
    pub struct ServerboundKeepAlive(pub KeepAlive);

    #[derive(Clone, Debug, Encode, Decode, From, Packet)]
    #[packet(id=0x05, state=Configuration)]
    pub struct ServerboundPong(Pong);

    #[derive(Clone, Debug, Encode, Decode, From, Packet)]
    #[packet(id=0x06, state=Configuration)]
    pub struct ServerboundResourcePack(ResourcePack);

    #[derive(Clone, Debug, Encode, Decode, From, Packet)]
    #[packet(id=0x07, state=Configuration)]
    pub struct ServerboundSelectKnownPacks<'a> {
        pub known_packs: Vec<KnownPack<'a>>,
    }

    #[derive(Clone, Debug, Encode, Decode, From, Packet)]
    #[packet(id=0x08, state=Configuration)]
    pub struct ServerboundCustomClickAction<'a>(pub CustomClickAction<'a>);

    #[derive(Clone, Debug, Encode, Decode, From, Packet)]
    #[packet(id=0x09, state=Configuration)]
    pub struct ServerboundAcceptCodeOfConduct;

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub enum Packet<'a> {
        ClientInformation(ServerboundClientInformation<'a>),
        CookieResponse(ServerboundCookieResponse<'a>),
        CustomPayload(ServerboundCustomPayload<'a>),
        FinishConfiguration,
        KeepAlive(KeepAlive),
        Pong(Pong),
        ResourcePack(ResourcePack),
        SelectKnownPacks(ServerboundSelectKnownPacks<'a>),
        CustomClickAction(ServerboundCustomClickAction<'a>),
        AcceptCodeOfConduct,
    }
}
