pub mod serverbound {
    use crate::Bounded;
    use derive_more::Into;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_protocol_macros::{Decode, Encode};
    use std::borrow::Cow;

    pub const MAX_COOKIE_PAYLOAD: usize = 5 * 1024;

    #[derive(Clone, Debug, PartialEq, Encode, Decode, Into)]
    pub struct CookieResponse<'a> {
        pub key: ResourceLocation<Cow<'a, str>>,
        pub payload: Option<Bounded<&'a [u8], MAX_COOKIE_PAYLOAD>>,
    }
}

pub mod clientbound {
    use derive_more::Into;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_protocol_macros::{Decode, Encode};
    use std::borrow::Cow;

    #[derive(Clone, Debug, Encode, Decode, Into)]
    pub struct CookieRequest<'a> {
        pub key: Option<ResourceLocation<Cow<'a, str>>>,
    }
}
