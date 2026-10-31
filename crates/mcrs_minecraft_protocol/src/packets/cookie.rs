pub mod serverbound {
    use crate::Bounded;
    use derive_more::Into;
    use mcrs_minecraft_resource::ResourceLocation;
    use mcrs_minecraft_wire::{Decode, Encode};

    pub const MAX_COOKIE_PAYLOAD: usize = 5 * 1024;

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Into)]
    pub struct CookieResponse<'a> {
        pub key: ResourceLocation<'a>,
        pub payload: Option<Bounded<&'a [u8], MAX_COOKIE_PAYLOAD>>,
    }
}

pub mod clientbound {
    use derive_more::Into;
    use mcrs_minecraft_resource::ResourceLocation;
    use mcrs_minecraft_wire::{Decode, Encode};

    #[derive(Clone, Debug, Encode, Decode, Into)]
    pub struct CookieRequest<'a> {
        pub key: Option<ResourceLocation<'a>>,
    }
}
