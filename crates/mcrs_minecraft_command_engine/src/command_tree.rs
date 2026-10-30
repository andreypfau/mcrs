use crate::command_set::CommandSet;
use crate::registrar::Handler;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bytes::Bytes;
use mcrs_command::{Graph, GraphBuilder};
use mcrs_minecraft_command::{CommandLevel, NoLookup, PermissionCheck, Source};
use mcrs_minecraft_engine::ability::PlayerOpLevel;
use mcrs_minecraft_engine::bus::{self, OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundCommands;
use mcrs_minecraft_protocol::{Packet, anyhow};

/// The command graph. The host owns it and lends it to the world in which a line parses, gets
/// suggestions or runs; in every other world it is empty.
#[derive(Resource)]
pub struct CommandTree<S: CommandSet> {
    pub(crate) graph: Option<SetGraph<S>>,
}

/// The command graph of the set `S`.
pub type SetGraph<S> = Graph<<S as CommandSet>::Argument, PermissionCheck, Handler>;

impl<S: CommandSet> Default for CommandTree<S> {
    fn default() -> Self {
        Self { graph: None }
    }
}

impl<S: CommandSet> CommandTree<S> {
    /// The frozen graph, in the world it is lent to.
    pub fn graph(&self) -> Option<&SetGraph<S>> {
        self.graph.as_ref()
    }
}

/// The `commands` packet of each command level, which every dimension sends to
/// the players that join it.
#[derive(Resource, Clone, Debug)]
pub struct CommandTreePackets {
    by_level: [Bytes; 5],
}

impl CommandTreePackets {
    /// The command tree a player with `op_level` can see.
    pub fn to_player(&self, host: Entity, op_level: PlayerOpLevel) -> Option<OutboundPlayerPacket> {
        let packet = self.by_level.get(usize::from(op_level.clamped()))?.clone();
        Some(bus::to(host, PacketPayload::Commands(packet)).critical())
    }

    pub(crate) fn new<S: CommandSet>(graph: &SetGraph<S>) -> anyhow::Result<Self> {
        let mut by_level: [Bytes; 5] = Default::default();
        for (id, packet) in (0..).zip(&mut by_level) {
            let source = Source::new(CommandLevel::from(id), &NoLookup);
            let mut bytes = Vec::new();
            let tree = mcrs_minecraft_command::player_tree(graph, &source)?;
            ClientboundCommands {
                nodes: tree.nodes,
                root: tree.root,
            }
            .encode_with_id(&mut bytes)?;
            *packet = Bytes::from(bytes);
        }
        Ok(Self { by_level })
    }
}

/// The tree that plugins add commands to while the app builds.
#[derive(Resource)]
pub(crate) struct CommandTreeBuilder<S: CommandSet> {
    pub(crate) builder: GraphBuilder<S::Argument, PermissionCheck, Handler>,
    pub(crate) next: u16,
}
