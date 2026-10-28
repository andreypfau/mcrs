mod effect;
mod lookup;
mod register;
mod run;
mod seam;
mod targets;
pub mod vanilla;

pub use effect::{Facing, Teleport};
pub use register::{AppCommands, Call, CommandRegistrar, Handler, NodeRef, RegistrationError};
pub use run::{CommandEvent, CommandOutput};
pub(crate) use run::{system_chat, system_message};
pub use seam::run_pending_commands;
pub use targets::{CommandTargets, DimensionEntry, PlayerEntry};

use crate::world::entity::player::ability::PlayerOpLevel;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::system::{SystemId, SystemParam};
use bevy_math::DVec3;
use bytes::Bytes;
use mcrs_command::{Arg, Argument, Chain, Contains, GraphBuilder, Limits};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_command::{
    ArgumentError, CommandLevel, Identifier, PermissionCheck, Source, VanillaArgument,
    VanillaGraph, error_message,
};
use mcrs_minecraft_command::{NoLookup, player_tree};
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, to};
use mcrs_minecraft_entity::PLAYER_EYE_HEIGHT;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::world::dimension::DimensionTypeId;
use mcrs_minecraft_level::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol::packets::game::clientbound::LookAnchor;
use mcrs_minecraft_protocol::text::{Color, IntoText, Text};
use mcrs_minecraft_registries::CurrentRegistries;

/// The command a node runs: the dimension worlds resolve it to their own handler system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct HandlerKey(pub(crate) u16);

/// The command graph. The host owns it and lends it to the world in which a line parses, gets
/// suggestions or runs; in every other world it is empty.
#[derive(Resource, Debug, Default)]
pub struct CommandTree {
    graph: Option<VanillaGraph<Handler>>,
}

impl CommandTree {
    pub fn graph(&self) -> Option<&VanillaGraph<Handler>> {
        self.graph.as_ref()
    }
}

/// The `commands` packet of each command level (CMDP-TREE-002), which every dimension sends to
/// the players that join it.
#[derive(Resource, Clone, Debug)]
pub(crate) struct CommandTreePackets {
    by_level: [Bytes; 5],
}

impl CommandTreePackets {
    fn new(graph: &VanillaGraph<Handler>) -> mcrs_minecraft_protocol::anyhow::Result<Self> {
        let mut by_level: [Bytes; 5] = Default::default();
        for (id, packet) in (0..).zip(&mut by_level) {
            let source = Source::new(CommandLevel::from_id(id), &NoLookup);
            let mut bytes = Vec::new();
            player_tree(graph, &source).encode_with_id(&mut bytes)?;
            *packet = Bytes::from(bytes);
        }
        Ok(Self { by_level })
    }
}

#[derive(Resource)]
struct CommandTreeBuilder {
    builder: GraphBuilder<VanillaArgument, PermissionCheck, Handler>,
    next: u16,
}

/// Owns the command tree: plugins register commands while the app builds, and the tree
/// freezes when it finishes.
pub struct CommandPlugin;

impl Plugin for CommandPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CommandTreeBuilder {
            builder: GraphBuilder::new(),
            next: 0,
        });
        app.init_resource::<CommandLimits>();
        app.init_resource::<seam::CommandSeam>();
        app.init_resource::<seam::CommandQueue>();
        app.add_observer(run::queue_command_line)
            .add_observer(run::queue_suggestion_request)
            .add_observer(run::broadcast_to_operators);
    }

    fn finish(&self, app: &mut App) {
        let Some(tree) = app.world_mut().remove_resource::<CommandTreeBuilder>() else {
            return;
        };
        let graph = match tree.builder.freeze() {
            Ok(graph) => graph,
            Err(error) => {
                tracing::error!("the command tree does not freeze: {error}");
                return;
            }
        };
        match CommandTreePackets::new(&graph) {
            Ok(packets) => {
                app.insert_resource(CommandTree { graph: Some(graph) });
                app.add_dimension_plugin(DimCommandPlugin { packets });
            }
            Err(error) => tracing::error!("the command tree does not encode: {error:#}"),
        }
    }
}

/// The result of a command: its number, or the error the source receives.
pub type CommandResult = Result<i32, CommandFailure>;

#[derive(Clone, Debug, PartialEq)]
pub struct CommandFailure(pub Text);

impl CommandFailure {
    pub fn translate(key: &'static str, with: Vec<Text>) -> Self {
        Self(Text::translate(key, with))
    }

    pub(crate) fn invalid_dimension(id: Identifier<'_>) -> Self {
        Self::translate(
            "argument.dimension.invalid",
            vec![id.to_string().into_text()],
        )
    }

    pub(crate) fn failed() -> Self {
        Self::translate("command.failed", vec![])
    }

    pub(crate) fn text(self) -> Text {
        self.0.color(Color::RED)
    }
}

/// The limits of the run of one command line (CMD-EX-036, CMD-EX-037), which the host reads
/// when it runs the line.
// chisle: the game rules `max_command_forks` and `max_command_sequence_length` have no state in
// the server yet; a game rule resource lifts it.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct CommandLimits(pub Limits);

/// One source of a command: who runs it, in which dimension, where, facing where, with which
/// permissions. A stage of `execute` maps it to other sources; the output stays the player who
/// sent the line.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceState {
    pub level: CommandLevel,
    /// The host anchor of the player the source holds, which names the player in every
    /// dimension, so the source keeps it when the player changes dimension (DIM-CHG-001).
    pub entity: Option<Entity>,
    /// The label of the source dimension. The source entity can be in another one
    /// (ARG-ENT-021).
    pub dimension: Entity,
    pub position: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub anchor: LookAnchor,
}

/// One run of a stage for one source: the host anchor of the player who receives the output,
/// the source, and the stage whose nodes the handler reads.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub output: Entity,
    pub source: SourceState,
    stage: usize,
    branch: u16,
}

impl SourceState {
    /// The source of the player with the host anchor `anchor`, in the dimension `dimension`.
    pub fn of_player(
        anchor: Entity,
        dimension: Entity,
        level: CommandLevel,
        transform: &Transform,
    ) -> Self {
        Self {
            level,
            entity: Some(anchor),
            dimension,
            position: transform.translation,
            yaw: transform.rotation.yaw(),
            pitch: transform.rotation.pitch(),
            anchor: LookAnchor::Feet,
        }
    }
}

/// The command line that runs in this world, and its chain. The host lends it to the world a
/// stage runs in.
#[derive(Resource, Default)]
pub(crate) struct RunningCommand {
    line: String,
    chain: Chain<Handler>,
}

/// The command tree a player with `op_level` can see (CMDP-TREE-002).
pub(crate) fn tree_packet(
    packets: &CommandTreePackets,
    host: Entity,
    op_level: PlayerOpLevel,
) -> Option<OutboundPlayerPacket> {
    let packet = packets
        .by_level
        .get(usize::from(op_level.clamped()))?
        .clone();
    Some(to(host, PacketPayload::Commands(packet)).critical())
}

/// What a modifier gives for one source.
#[derive(Clone, Debug, PartialEq)]
pub enum Modified {
    None,
    One(SourceState),
    Many(Vec<SourceState>),
}

pub type ModifyResult = Result<Modified, CommandFailure>;

/// What a handler reads its arguments with.
#[derive(SystemParam)]
pub struct CommandContext<'w, 's> {
    tree: Res<'w, CommandTree>,
    running: Res<'w, RunningCommand>,
    targets: Res<'w, CommandTargets>,
    registries: Option<Res<'w, CurrentRegistries>>,
    blocks: Option<Res<'w, Blocks>>,
    dimension_type: Query<'w, 's, &'static DimensionTypeId>,
}

impl<'w, 's> CommandContext<'w, 's> {
    /// The source of the call, which reads the world through this context.
    pub fn source(&self, call: &Invocation) -> Result<Source<'_, Self>, CommandFailure> {
        self.source_of(&call.source)
    }

    /// The source of `state`, with the rank of its dimension in the dimension order. The host
    /// fills the targets before a line parses, so a dimension they lack fails the source.
    pub(crate) fn source_of(
        &self,
        state: &SourceState,
    ) -> Result<Source<'_, Self>, CommandFailure> {
        let Some(dimension) = self.targets.dimension_of(state.dimension) else {
            tracing::error!(dimension = ?state.dimension, "a command source names no running dimension");
            return Err(CommandFailure::failed());
        };
        Ok(Source {
            position: state.position,
            yaw: state.yaw,
            pitch: state.pitch,
            anchor: state.anchor,
            eye_height: Some(PLAYER_EYE_HEIGHT),
            dimension,
            ..Source::new(state.level, self)
        })
    }

    /// The dimensions that run and the players of every one of them.
    pub fn targets(&self) -> &CommandTargets {
        &self.targets
    }

    /// The command line that runs.
    pub fn line(&self) -> &str {
        &self.running.line
    }

    /// The value of an argument the branch of the call always holds.
    pub fn arg<'a, T>(
        &'a self,
        call: &Invocation,
        arg: Arg<T>,
    ) -> Result<T::Value<'a>, CommandFailure>
    where
        VanillaArgument: Contains<T>,
        T: Argument<Source<'a, Self>>,
        ArgumentError: From<T::Error>,
    {
        self.opt(call, arg)?.ok_or_else(CommandFailure::failed)
    }

    /// The value of an argument the branch of the call may lack.
    pub fn opt<'a, T>(
        &'a self,
        call: &Invocation,
        arg: Arg<T>,
    ) -> Result<Option<T::Value<'a>>, CommandFailure>
    where
        VanillaArgument: Contains<T>,
        T: Argument<Source<'a, Self>>,
        ArgumentError: From<T::Error>,
    {
        let Some(span) = self.running.chain.span(call.stage, arg.node()) else {
            return Ok(None);
        };
        let graph = self.tree.graph().ok_or_else(CommandFailure::failed)?;
        let source = self.source(call)?;
        let line = self.line();
        match graph.value(line, span, arg, &source) {
            None => Err(CommandFailure::failed()),
            Some(Ok(value)) => Ok(Some(value)),
            Some(Err(error)) => Err(CommandFailure(error_message(line, &error.map_into()))),
        }
    }
}

/// The handlers of one output type, indexed by [`HandlerKey`].
#[derive(Resource)]
struct Handlers<O: Send + Sync + 'static>(Vec<Option<SystemId<In<Invocation>, O>>>);

impl<O: Send + Sync + 'static> Default for Handlers<O> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<O: Send + Sync + 'static> Handlers<O> {
    fn get(&self, key: HandlerKey) -> Option<SystemId<In<Invocation>, O>> {
        self.0.get(usize::from(key.0)).copied().flatten()
    }
}

type CommandHandlers = Handlers<CommandResult>;
type ModifierHandlers = Handlers<ModifyResult>;

/// Holds the handlers the host runs in a dimension world, the resources the host lends to it,
/// and the `commands` packets the world sends.
#[derive(Clone)]
pub(crate) struct DimCommandPlugin {
    pub packets: CommandTreePackets,
}

impl Plugin for DimCommandPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.packets.clone());
        app.init_resource::<CommandTree>();
        app.init_resource::<CommandHandlers>();
        app.init_resource::<ModifierHandlers>();
        app.init_resource::<RunningCommand>();
        app.init_resource::<CommandTargets>();
        app.init_resource::<effect::CommandOutbox>();
    }
}

pub(crate) fn level_of(op_level: PlayerOpLevel) -> CommandLevel {
    CommandLevel::from_id(op_level.clamped())
}
