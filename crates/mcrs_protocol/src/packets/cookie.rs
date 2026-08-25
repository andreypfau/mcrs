pub mod serverbound {
    use derive_more::Into;
    use mcrs_ident::Ident;
    use mcrs_protocol_macros::{Decode, Encode};
    use std::borrow::Cow;

    #[derive(Clone, Debug, Encode, Decode, Into)]
    pub struct CookieResponse<'a> {
        pub key: Option<Ident<Cow<'a, str>>>,
        pub cookie: &'a [u8],
    }
}

pub mod clientbound {
    use derive_more::Into;
    use mcrs_ident::Ident;
    use mcrs_protocol_macros::{Decode, Encode};
    use std::borrow::Cow;

    #[derive(Clone, Debug, Encode, Decode, Into)]
    pub struct CookieRequest<'a> {
        pub key: Option<Ident<Cow<'a, str>>>,
    }
}
