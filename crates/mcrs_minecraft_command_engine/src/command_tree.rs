use crate::command_set::CommandSet;
use crate::registrar::Handler;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bytes::Bytes;
use mcrs_command::{Graph, GraphBuilder};
use mcrs_minecraft_command::{CommandLevel, NoLookup, PermissionCheck, Source};
use mcrs_minecraft_engine::ability::{OpLevelAnnounced, PlayerOpLevel};
use mcrs_minecraft_engine::bus::{self, OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_protocol::{Packet, anyhow};
use mcrs_minecraft_protocol_game::clientbound::ClientboundCommands;

#[derive(Resource)]
pub struct CommandTree<S: CommandSet> {
    pub(crate) graph: SetGraph<S>,
}

pub type SetGraph<S> = Graph<<S as CommandSet>::Argument, PermissionCheck, Handler>;

impl<S: CommandSet> CommandTree<S> {
    pub fn graph(&self) -> &SetGraph<S> {
        &self.graph
    }
}

#[derive(Resource, Clone, Debug)]
pub struct CommandTreePackets {
    by_level: [Bytes; 5],
}

impl CommandTreePackets {
    pub fn to_player(
        &self,
        session: PlayerSession,
        op_level: PlayerOpLevel,
    ) -> Option<OutboundPlayerPacket> {
        let packet = self.by_level.get(usize::from(op_level.clamped()))?.clone();
        Some(bus::to(session, PacketPayload::Commands(packet)))
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

pub(crate) fn send_command_tree(
    mut announced: MessageReader<OpLevelAnnounced>,
    packets: Res<CommandTreePackets>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    for announced in announced.read() {
        if let Some(packet) = packets.to_player(announced.session, announced.level) {
            packet_writer.write(packet);
        }
    }
}

#[derive(Resource)]
pub(crate) struct CommandTreeBuilder<S: CommandSet> {
    pub(crate) builder: GraphBuilder<S::Argument, PermissionCheck, Handler>,
    pub(crate) next: u16,
}
