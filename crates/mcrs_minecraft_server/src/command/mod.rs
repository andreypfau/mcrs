mod dim;
mod effect;
mod lookup;
mod register;
mod run;
mod seam;
mod targets;
pub mod vanilla;

pub use dim::DebugCommandsPlugin;
pub use effect::{Effect, Facing, Teleport};
pub use register::{AppCommands, Call, CommandRegistrar, Handler, NodeRef, RegistrationError};
pub(crate) use run::system_message;
pub use run::{CommandEvent, CommandOutput};
pub use seam::run_pending_commands;
pub use targets::{CommandTargets, DimensionEntry, PlayerEntry};

use crate::world::entity::player::ability::PlayerOpLevel;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::system::{SystemId, SystemParam};
use bevy_math::DVec3;
use mcrs_command::{Arg, Argument, Chain, Contains, GraphBuilder, Limits};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_command::{
    ArgumentError, CommandLevel, CommandSyntaxError, PermissionCheck, Source, VanillaArgument,
    VanillaGraph, error_message,
};
use mcrs_minecraft_command::{NoLookup, player_tree};
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, to};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_entity::PLAYER_EYE_HEIGHT;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::world::dimension::DimensionTypeId;
use mcrs_minecraft_level::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_protocol::packets::game::clientbound::LookAnchor;
use mcrs_minecraft_protocol::text::{Color, IntoText, Text};
use mcrs_minecraft_registries::CurrentRegistries;

/// The command a node runs: the dimension worlds resolve it to their own handler system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct HandlerKey(pub(crate) u16);

/// Each dimension world holds a copy.
#[derive(Resource, Clone, Debug)]
pub struct CommandTree(pub VanillaGraph<Handler>);

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
        app.add_observer(run::reject_illegal_characters)
            .add_observer(run::broadcast_to_operators);
    }

    fn finish(&self, app: &mut App) {
        let Some(tree) = app.world_mut().remove_resource::<CommandTreeBuilder>() else {
            return;
        };
        match tree.builder.freeze() {
            Ok(graph) => {
                app.add_dimension_plugin(DimCommandPlugin {
                    tree: CommandTree(graph),
                });
            }
            Err(error) => tracing::error!("the command tree does not freeze: {error}"),
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

    pub(crate) fn failed() -> Self {
        Self::translate("command.failed", vec![])
    }

    pub(crate) fn syntax(line: &str, error: &CommandSyntaxError) -> Self {
        Self(error_message(line, error))
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

    /// `dimension` is the rank of the source dimension in the dimension order.
    pub(crate) fn source<'s, L>(&self, lookup: &'s L, dimension: usize) -> Source<'s, L> {
        Source {
            position: self.position,
            yaw: self.yaw,
            pitch: self.pitch,
            anchor: self.anchor,
            eye_height: Some(PLAYER_EYE_HEIGHT),
            dimension,
            ..Source::new(self.level, lookup)
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
    tree: &VanillaGraph<Handler>,
    host: Entity,
    op_level: PlayerOpLevel,
) -> OutboundPlayerPacket {
    let source = Source::new(level_of(op_level), &NoLookup);
    to(host, PacketPayload::Commands(player_tree(tree, &source))).critical()
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
    profiles: Query<'w, 's, &'static GameProfile, With<Player>>,
    dimension_type: Query<'w, 's, &'static DimensionTypeId>,
}

impl<'w, 's> CommandContext<'w, 's> {
    /// The source of the call, which reads the world through this context.
    pub fn source(&self, call: &Invocation) -> Source<'_, Self> {
        self.source_of(&call.source)
    }

    /// The source of `state`, with the rank of its dimension in the dimension order.
    pub(crate) fn source_of(&self, state: &SourceState) -> Source<'_, Self> {
        let dimension = self
            .targets
            .dimension_of(state.dimension)
            .unwrap_or_default();
        state.source(self, dimension)
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
        let source = self.source(call);
        let line = self.line();
        match self.tree.0.value(line, span, arg, &source) {
            None => Err(CommandFailure::failed()),
            Some(Ok(value)) => Ok(Some(value)),
            Some(Err(error)) => Err(CommandFailure::syntax(line, &error.map_into())),
        }
    }
}

#[derive(Resource, Default)]
struct CommandHandlers(Vec<Option<SystemId<In<Invocation>, CommandResult>>>);

impl CommandHandlers {
    fn get(&self, key: HandlerKey) -> Option<SystemId<In<Invocation>, CommandResult>> {
        self.0.get(usize::from(key.0)).copied().flatten()
    }
}

#[derive(Resource, Default)]
struct ModifierHandlers(Vec<Option<SystemId<In<Invocation>, ModifyResult>>>);

impl ModifierHandlers {
    fn get(&self, key: HandlerKey) -> Option<SystemId<In<Invocation>, ModifyResult>> {
        self.0.get(usize::from(key.0)).copied().flatten()
    }
}

/// Parses and suggests commands in a dimension world, against the tree the host froze, and
/// holds the handlers the host runs there.
#[derive(Clone)]
pub(crate) struct DimCommandPlugin {
    pub tree: CommandTree,
}

impl Plugin for DimCommandPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.tree.clone());
        app.init_resource::<CommandHandlers>();
        app.init_resource::<ModifierHandlers>();
        app.init_resource::<RunningCommand>();
        app.init_resource::<CommandTargets>();
        app.init_resource::<effect::CommandOutbox>();
        app.init_resource::<seam::PendingLines>();
        app.add_observer(run::handle_chat_command);
        app.register_required_components::<Player, run::PendingSuggestion>();
        app.add_observer(run::queue_suggestion_request);
        app.add_systems(
            bevy_app::FixedUpdate,
            run::answer_suggestion_requests
                .in_set(mcrs_minecraft_level::tick::TickSet::Output)
                .before(crate::world::entity::player::persistence::autosave_players)
                .run_if(run::has_pending_suggestion),
        );
        app.add_systems(
            bevy_app::Update,
            run::send_command_tree
                .after(crate::world::entity::player::send_op_level)
                .before(crate::world::entity::player::game_mode::send_game_mode),
        );
    }
}

pub(crate) fn level_of(op_level: PlayerOpLevel) -> CommandLevel {
    CommandLevel::from_id(op_level.clamped())
}
