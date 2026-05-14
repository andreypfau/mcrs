pub mod clientbound {
    use crate::PacketSide;
    use derive_more::Into;
    use mcrs_protocol_macros::{Decode, Encode, Packet};

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, Into, Packet)]
    #[packet(id = 0x01, state = Status, side = PacketSide::Clientbound)]
    pub struct PongResponse {
        pub payload: u64,
    }
}

pub mod serverbound {
    use crate::PacketSide;
    use derive_more::Into;
    use mcrs_protocol_macros::{Decode, Encode, Packet};

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, Into, Packet)]
    #[packet(id = 0x01, state = Status, side = PacketSide::Serverbound)]
    pub struct PingRequest {
        pub payload: u64,
    }
}
