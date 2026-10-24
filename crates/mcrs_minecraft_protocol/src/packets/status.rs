pub mod clientbound {
    use derive_more::Into;
    use mcrs_minecraft_wire::{Decode, Encode};

    pub use crate::packets::ping::clientbound::PongResponse;

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, Into)]
    pub struct StatusResponse<'a> {
        pub json: &'a str,
    }
}

pub mod serverbound {
    use mcrs_minecraft_wire::{Decode, Encode};

    pub use crate::packets::ping::serverbound::PingRequest;

    #[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
    pub struct StatusRequest;
}
