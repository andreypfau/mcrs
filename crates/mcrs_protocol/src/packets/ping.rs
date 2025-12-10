pub mod clientbound {
    use derive_more::Into;
    use mcrs_protocol_macros::{Decode, Encode};

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, Into)]
    pub struct PongResponse {
        pub payload: u64,
    }
}

pub mod serverbound {
    use derive_more::Into;
    use mcrs_protocol_macros::{Decode, Encode};

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, Into)]
    pub struct PingRequest {
        pub payload: u64,
    }
}
