pub mod clientbound {
    use crate::PacketSide;
    use crate::packets::ping::clientbound::PongResponse;
    use derive_more::{From, Into};
    use mcrs_protocol_macros::{Decode, Encode, Packet};

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, Into, Packet)]
    #[packet(id = 0x00, state = Status, side = PacketSide::Clientbound)]
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
    use crate::PacketSide;
    use crate::packets::ping::serverbound::PingRequest;
    use derive_more::From;
    use mcrs_protocol_macros::{Decode, Encode, Packet};

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, Packet)]
    #[packet(id = 0x00, state = Status, side = PacketSide::Serverbound)]
    pub struct StatusRequest;

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, From)]
    pub enum Packet {
        StatusRequest(StatusRequest),
        PingRequest(PingRequest),
    }
}
