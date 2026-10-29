use crate::registrar::Handler;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bytes::Bytes;
use mcrs_command::GraphBuilder;
use mcrs_minecraft_command::{CommandLevel, NoLookup, PermissionCheck, Source};
use mcrs_minecraft_command_vanilla::{VanillaArgument, VanillaGraph, player_tree};
use mcrs_minecraft_engine::ability::PlayerOpLevel;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, to};
use mcrs_minecraft_protocol::Packet;

/// The command graph. The host owns it and lends it to the world in which a line parses, gets
/// suggestions or runs; in every other world it is empty.
#[derive(Resource, Debug, Default)]
pub struct CommandTree {
    pub(crate) graph: Option<VanillaGraph<Handler>>,
}

impl CommandTree {
    pub fn graph(&self) -> Option<&VanillaGraph<Handler>> {
        self.graph.as_ref()
    }
}

/// The `commands` packet of each command level (CMDP-TREE-002), which every dimension sends to
/// the players that join it.
#[derive(Resource, Clone, Debug)]
pub struct CommandTreePackets {
    by_level: [Bytes; 5],
}

impl CommandTreePackets {
    /// The command tree a player with `op_level` can see (CMDP-TREE-002).
    pub fn to_player(&self, host: Entity, op_level: PlayerOpLevel) -> Option<OutboundPlayerPacket> {
        let packet = self.by_level.get(usize::from(op_level.clamped()))?.clone();
        Some(to(host, PacketPayload::Commands(packet)).critical())
    }

    pub(crate) fn new(
        graph: &VanillaGraph<Handler>,
    ) -> mcrs_minecraft_protocol::anyhow::Result<Self> {
        let mut by_level: [Bytes; 5] = Default::default();
        for (id, packet) in (0..).zip(&mut by_level) {
            let source = Source::new(CommandLevel::from(id), &NoLookup);
            let mut bytes = Vec::new();
            player_tree(graph, &source)?.encode_with_id(&mut bytes)?;
            *packet = Bytes::from(bytes);
        }
        Ok(Self { by_level })
    }
}

/// The tree that plugins add commands to while the app builds.
#[derive(Resource)]
pub(crate) struct CommandTreeBuilder {
    pub(crate) builder: GraphBuilder<VanillaArgument, PermissionCheck, Handler>,
    pub(crate) next: u16,
}
