pub mod serverbound {
    use crate::handshake::Intent;
    use crate::{Bounded, VarInt};
    use mcrs_minecraft_protocol_macros::{Decode, Encode};

    #[derive(Clone, Debug, PartialEq, Encode, Decode)]
    pub struct ServerboundHandshake<'a> {
        pub protocol_version: VarInt,
        pub server_address: Bounded<&'a str, 1024>,
        pub server_port: u16,
        pub intent: Intent,
    }
}
