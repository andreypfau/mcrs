pub mod serverbound {
    use std::borrow::Cow;
    use derive_more::Into;
    use valence_ident::Ident;
    use mcrs_protocol_macros::{Decode, Encode};

    #[derive(Clone, Debug, Encode, Decode, Into)]
    pub struct CookieResponse<'a> {
        pub key: Option<Ident<Cow<'a, str>>>,
        pub cookie: &'a [u8],
    }
}

pub mod clientbound {
    use std::borrow::Cow;
    use derive_more::Into;
    use valence_ident::Ident;
    use mcrs_protocol_macros::{Decode, Encode};

    #[derive(Clone, Debug, Encode, Decode, Into)]
    pub struct CookieRequest<'a> {
        pub key: Option<Ident<Cow<'a, str>>>,
    }
}