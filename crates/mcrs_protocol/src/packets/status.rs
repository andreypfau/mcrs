pub mod clientbound {
    use crate::packets::ping::clientbound::PongResponse;
    use derive_more::{From, Into};
    use mcrs_protocol_macros::{Decode, Encode};

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, Into)]
    pub struct StatusResponse<'a> {
        pub json: &'a str,
    }

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, From)]
    pub enum Packet<'a> {
        StatusResponse(StatusResponse<'a>),
        PongResponse(PongResponse),
    }
}

pub mod serverbound {
    use crate::packets::ping::serverbound::PingRequest;
    use derive_more::From;
    use mcrs_protocol_macros::{Decode, Encode};

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, From)]
    pub enum Packet {
        StatusRequest,
        PingRequest(PingRequest),
    }
}
