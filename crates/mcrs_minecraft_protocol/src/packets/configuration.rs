pub use self::clientbound::ClientboundCustomPayload;
pub use self::clientbound::ClientboundDisconnect;
pub use self::clientbound::ClientboundFinishConfiguration;
pub use self::clientbound::ClientboundKeepAlive;
pub use self::clientbound::ClientboundRegistryData;
pub use self::clientbound::ClientboundShowDialog;
pub use self::clientbound::ClientboundUpdateTags;

pub mod clientbound {
    use crate::packets::common::clientbound::{KeepAlive, Payload};
    use derive_more::From;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_nbt::compound::NbtCompound;
    use mcrs_minecraft_protocol_macros::{Decode, Encode};
    use std::borrow::Cow;

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub struct ClientboundCustomPayload<'a>(pub Payload<'a>);

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundDisconnect {
        pub reason: crate::Text,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundFinishConfiguration;

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundKeepAlive(pub KeepAlive);

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundRegistryData<'a> {
        pub registry: ResourceLocation<Cow<'a, str>>,
        pub entries: Vec<crate::registry::Entry<'a>>,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundSelectKnownPacks<'a> {
        pub known_packs: Vec<crate::resource_pack::KnownPack<'a>>,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundShowDialog {
        pub dialog: NbtCompound,
    }

    /// A single tag within a registry, containing the tag name and entry IDs.
    #[derive(Clone, Debug, Encode, Decode)]
    pub struct TagGroup<'a> {
        /// The tag identifier (e.g., "minecraft:mineable/pickaxe")
        pub name: ResourceLocation<Cow<'a, str>>,
        /// Array of numeric registry entry IDs that belong to this tag
        pub entries: Vec<crate::VarInt>,
    }

    /// Tags for a specific registry type.
    #[derive(Clone, Debug, Encode, Decode)]
    pub struct RegistryTags<'a> {
        /// The registry identifier (e.g., "minecraft:block", "minecraft:item")
        pub registry: ResourceLocation<Cow<'a, str>>,
        /// Array of tags for this registry
        pub tags: Vec<TagGroup<'a>>,
    }

    /// Packet sent during Configuration phase to synchronize tags with the client.
    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundUpdateTags<'a> {
        /// Tags grouped by registry type
        pub registries: Vec<RegistryTags<'a>>,
    }
}

pub mod serverbound {
    use crate::packets::common::serverbound::{
        ClientInformation, CustomClickAction, KeepAlive, Pong, ResourcePack,
    };
    use crate::resource_pack::KnownPack;
    use derive_more::From;
    use mcrs_minecraft_protocol_macros::{Decode, Encode};

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub struct ServerboundClientInformation<'a>(pub ClientInformation<'a>);

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub struct ServerboundCookieResponse<'a>(
        crate::packets::cookie::serverbound::CookieResponse<'a>,
    );

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub struct ServerboundCustomPayload<'a>(crate::packets::common::serverbound::Payload<'a>);

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub struct ServerboundFinishConfiguration;

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub struct ServerboundKeepAlive(pub KeepAlive);

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub struct ServerboundPong(Pong);

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub struct ServerboundResourcePack(ResourcePack);

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub struct ServerboundSelectKnownPacks<'a> {
        pub known_packs: Vec<KnownPack<'a>>,
    }

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub struct ServerboundCustomClickAction<'a>(pub CustomClickAction<'a>);

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub struct ServerboundAcceptCodeOfConduct;
}
