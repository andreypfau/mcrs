pub mod serverbound {
    use derive_more::{From, Into};
    use crate::handshake::Intent;
    use crate::{Bounded, VarInt};
    use mcrs_protocol_macros::{Decode, Encode, Packet};

    #[derive(Clone, Debug, Encode, Decode, Packet)]
    #[packet(id=0x00, state=Handshaking)]
    pub struct ServerboundHandshake<'a> {
        pub protocol_version: VarInt,
        pub server_address: Bounded<&'a str, 255>,
        pub server_port: u16,
        pub intent: Intent,
    }

    #[derive(Clone, Debug, Encode, Decode, From)]
    pub enum Packet<'a> {
        Handshake(ServerboundHandshake<'a>),
    }
}
