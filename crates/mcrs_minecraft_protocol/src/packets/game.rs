pub mod clientbound {
    use crate::packets::common::clientbound::KeepAlive;
    use mcrs_minecraft_text::Text;
    use mcrs_minecraft_wire::{Decode, Encode};

    /// Opens or closes a bundle: the client handles the packets between two delimiters in one
    /// pass.
    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundBundleDelimiter;

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundDisconnect {
        pub reason: Text,
    }

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundKeepAlive(pub KeepAlive);

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ClientboundStartConfiguration;
}

pub mod serverbound {
    use crate::packets::common::serverbound::KeepAlive;
    use mcrs_minecraft_wire::{Decode, Encode};

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundConfigurationAcknowledged;

    #[derive(Clone, Debug, Encode, Decode)]
    pub struct ServerboundKeepAlive(pub KeepAlive);
}
