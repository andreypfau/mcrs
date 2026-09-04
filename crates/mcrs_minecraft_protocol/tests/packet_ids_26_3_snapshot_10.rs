//! Pins every packet id modelled by this crate to its 26.3-snapshot-10 wire value.
//!
//! Vanilla assigns ids by registration order in the protocol builders, so a wrong
//! id is self-consistent between our own client and server and only shows up
//! against a real server. The values below were derived by walking the
//! `addPacket`/`withBundlePacket` chains of `HandshakeProtocols`, `StatusProtocols`,
//! `LoginProtocols`, `ConfigurationProtocols` and `GameProtocols`, counting the
//! packets registered through the shared `common`, `cookie` and `ping` helpers,
//! and the bundle delimiter that occupies play/clientbound slot 0.

use mcrs_minecraft_protocol::packets::{configuration, game, intent, login, ping, status};
use mcrs_minecraft_protocol::{ConnectionState, Packet, PacketSide};

fn assert_packet<P: Packet>(id: i32, side: PacketSide, state: ConnectionState) {
    assert_eq!(P::ID, id, "{} has the wrong id", P::NAME);
    assert_eq!(P::SIDE, side, "{} has the wrong side", P::NAME);
    assert_eq!(P::STATE, state, "{} has the wrong state", P::NAME);
}

macro_rules! pinned {
    ($($id:expr => $ty:ty, $side:ident, $state:ident;)*) => {
        $(assert_packet::<$ty>($id, PacketSide::$side, ConnectionState::$state);)*
    };
}

#[test]
fn handshake_and_status_ids() {
    pinned! {
        0x00 => intent::serverbound::ServerboundHandshake<'_>, Serverbound, Handshaking;
        0x00 => status::serverbound::StatusRequest, Serverbound, Status;
        0x01 => ping::serverbound::PingRequest, Serverbound, Status;
        0x00 => status::clientbound::StatusResponse<'_>, Clientbound, Status;
        0x01 => ping::clientbound::PongResponse, Clientbound, Status;
    }
}

#[test]
fn login_ids() {
    pinned! {
        0x00 => login::clientbound::ClientboundLoginDisconnect<'_>, Clientbound, Login;
        0x01 => login::clientbound::ClientboundHello<'_>, Clientbound, Login;
        0x02 => login::clientbound::ClientboundLoginFinished<'_>, Clientbound, Login;
        0x03 => login::clientbound::LoginCompression, Clientbound, Login;

        0x00 => login::serverbound::ServerboundHello<'_>, Serverbound, Login;
        0x01 => login::serverbound::ServerboundKey<'_>, Serverbound, Login;
        0x02 => login::serverbound::ServerboundCustomQueryAnswer<'_>, Serverbound, Login;
        0x03 => login::serverbound::ServerboundLoginAcknowledged, Serverbound, Login;
        0x04 => login::serverbound::ServerboundCookieResponse<'_>, Serverbound, Login;
    }
}

#[test]
fn configuration_ids() {
    pinned! {
        0x03 => configuration::clientbound::ClientboundFinishConfiguration, Clientbound, Configuration;
        0x04 => configuration::clientbound::ClientboundKeepAlive, Clientbound, Configuration;
        0x07 => configuration::clientbound::ClientboundRegistryData<'_>, Clientbound, Configuration;
        0x0E => configuration::clientbound::ClientboundUpdateTags<'_>, Clientbound, Configuration;
        0x0F => configuration::clientbound::ClientboundSelectKnownPacks<'_>, Clientbound, Configuration;
        0x13 => configuration::clientbound::ClientboundShowDialog, Clientbound, Configuration;

        0x00 => configuration::serverbound::ServerboundClientInformation<'_>, Serverbound, Configuration;
        0x01 => configuration::serverbound::ServerboundCookieResponse<'_>, Serverbound, Configuration;
        0x02 => configuration::serverbound::ServerboundCustomPayload<'_>, Serverbound, Configuration;
        0x03 => configuration::serverbound::ServerboundFinishConfiguration, Serverbound, Configuration;
        0x04 => configuration::serverbound::ServerboundKeepAlive, Serverbound, Configuration;
        0x05 => configuration::serverbound::ServerboundPong, Serverbound, Configuration;
        0x06 => configuration::serverbound::ServerboundResourcePack, Serverbound, Configuration;
        0x07 => configuration::serverbound::ServerboundSelectKnownPacks<'_>, Serverbound, Configuration;
        0x08 => configuration::serverbound::ServerboundCustomClickAction<'_>, Serverbound, Configuration;
        0x09 => configuration::serverbound::ServerboundAcceptCodeOfConduct, Serverbound, Configuration;
    }
}

#[test]
fn play_clientbound_ids() {
    use game::clientbound::*;
    pinned! {
        0x01 => ClientboundAddEntity, Clientbound, Game;
        0x05 => ClientboundBlockDestruction, Clientbound, Game;
        0x08 => ClientboundBlockUpdate, Clientbound, Game;
        0x0B => ClientboundChunkBatchFinished, Clientbound, Game;
        0x0C => ClientboundChunkBatchStart, Clientbound, Game;
        0x12 => ClientboundContainerSetContent, Clientbound, Game;
        0x20 => ClientboundDisconnect, Clientbound, Game;
        0x22 => ClientboundEntityEvent, Clientbound, Game;
        0x23 => ClientboundEntityPositionSync, Clientbound, Game;
        0x25 => ClientboundForgetLevelChunk, Clientbound, Game;
        0x26 => ClientboundGameEvent, Clientbound, Game;
        0x2C => ClientboundKeepAlive, Clientbound, Game;
        0x2D => ClientboundLevelChunkWithLight<'_>, Clientbound, Game;
        0x30 => ClientboundLightUpdate<'_>, Clientbound, Game;
        0x31 => ClientboundLogin<'_>, Clientbound, Game;
        0x35 => ClientboundMoveEntityPos, Clientbound, Game;
        0x36 => ClientboundMoveEntityPosRot, Clientbound, Game;
        0x37 => ClientboundMoveMinecartAlongTrack, Clientbound, Game;
        0x38 => ClientboundMoveEntityRot, Clientbound, Game;
        0x46 => ClientboundPlayerInfoUpdate<'_>, Clientbound, Game;
        0x48 => ClientboundPlayerPosition, Clientbound, Game;
        0x4D => ClientboundRemoveEntities, Clientbound, Game;
        0x53 => ClientboundRespawn<'_>, Clientbound, Game;
        0x54 => ClientboundRotateHead, Clientbound, Game;
        0x55 => ClientboundSectionBlocksUpdate<'_>, Clientbound, Game;
        0x5F => ClientboundSetChunkCacheCenter, Clientbound, Game;
        0x60 => ClientboundChunkCacheRadius, Clientbound, Game;
        0x77 => ClientboundStartConfiguration, Clientbound, Game;
        0x7B => ClientboundSystemChatPacket, Clientbound, Game;
    }
}

#[test]
fn play_serverbound_ids() {
    use game::serverbound::*;
    pinned! {
        0x00 => ServerboundAcceptTeleportation, Serverbound, Game;
        0x02 => ServerboundBlockEntityTagQuery, Serverbound, Game;
        0x03 => ServerboundSelectBundleItem, Serverbound, Game;
        0x04 => ServerboundChangeDifficulty, Serverbound, Game;
        0x05 => ServerboundChangeGameMode, Serverbound, Game;
        0x06 => ServerboundChatAck, Serverbound, Game;
        0x07 => ServerboundChatCommand<'_>, Serverbound, Game;
        0x08 => ServerboundChatCommandSigned<'_>, Serverbound, Game;
        0x09 => ServerboundChat<'_>, Serverbound, Game;
        0x0A => ServerboundChatSessionUpdate, Serverbound, Game;
        0x0B => ServerboundChunkBatchReceived, Serverbound, Game;
        0x0E => ServerboundClientInformation<'_>, Serverbound, Game;
        0x10 => ServerboundConfigurationAcknowledged, Serverbound, Game;
        0x12 => ServerboundContainerClick, Serverbound, Game;
        0x1C => ServerboundKeepAlive, Serverbound, Game;
        0x1E => ServerboundMovePlayerPos, Serverbound, Game;
        0x1F => ServerboundMovePlayerPosRot, Serverbound, Game;
        0x20 => ServerboundMovePlayerRot, Serverbound, Game;
        0x21 => ServerboundMovePlayerStatusOnly, Serverbound, Game;
        0x29 => ServerboundPlayerAction, Serverbound, Game;
        0x36 => ServerboundSetCarriedItem, Serverbound, Game;
        0x42 => ServerboundUseItemOn, Serverbound, Game;
    }
}
