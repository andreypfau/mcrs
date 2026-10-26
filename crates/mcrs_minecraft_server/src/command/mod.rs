mod dim;
mod lookup;
mod register;
mod run;
pub mod vanilla;

pub use dim::DebugCommandsPlugin;
pub use register::{AppCommands, Call, CommandRegistrar, Handler, NodeRef, RegistrationError};
pub(crate) use run::system_message;
pub use run::{CommandEvent, CommandFeedback};

use crate::world::entity::player::ability::PlayerOpLevel;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::system::{SystemId, SystemParam};
use mcrs_command::{Arg, Argument, Contains, GraphBuilder, NodeId, Span};
use mcrs_minecraft_command::{
    ArgumentError, CommandLevel, CommandSyntaxError, PermissionCheck, Source, VanillaArgument,
    VanillaGraph, error_message,
};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_entity::PLAYER_EYE_HEIGHT;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::world::sub_app::AddDimensionPlugins;
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

/// The source of a command at the moment it was sent.
#[derive(Clone, Copy, Debug)]
pub struct SourceState {
    pub level: CommandLevel,
    pub transform: Transform,
}

/// The command line a handler runs, with the nodes of the context that runs.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub player: Entity,
    pub source: SourceState,
    pub line: String,
    nodes: Vec<(NodeId, Span)>,
    branch: u16,
}

impl SourceState {
    pub(crate) fn source<'s, L>(&self, lookup: &'s L) -> Source<'s, L> {
        Source {
            position: self.transform.translation,
            yaw: self.transform.rotation.yaw(),
            pitch: self.transform.rotation.pitch(),
            eye_height: Some(PLAYER_EYE_HEIGHT),
            ..Source::new(self.level, lookup)
        }
    }
}

impl Invocation {
    fn span(&self, node: NodeId) -> Option<Span> {
        self.nodes
            .iter()
            .find(|&&(parsed, _)| parsed == node)
            .map(|&(_, span)| span)
    }
}

/// What a handler reads its arguments with.
#[derive(SystemParam)]
pub struct CommandContext<'w, 's> {
    tree: Res<'w, CommandTree>,
    registries: Option<Res<'w, CurrentRegistries>>,
    profiles: Query<'w, 's, &'static GameProfile, With<Player>>,
}

impl<'w, 's> CommandContext<'w, 's> {
    /// The source of the call, which reads the world through this context.
    pub fn source(&self, call: &Invocation) -> Source<'_, Self> {
        call.source.source(self)
    }

    /// The value of an argument the branch of the call always holds.
    pub fn arg<'a, 'c, T>(
        &'a self,
        call: &'c Invocation,
        arg: Arg<T>,
    ) -> Result<T::Value<'c>, CommandFailure>
    where
        VanillaArgument: Contains<T>,
        T: Argument<Source<'a, Self>>,
        ArgumentError: From<T::Error>,
    {
        self.opt(call, arg)?.ok_or_else(CommandFailure::failed)
    }

    /// The value of an argument the branch of the call may lack.
    pub fn opt<'a, 'c, T>(
        &'a self,
        call: &'c Invocation,
        arg: Arg<T>,
    ) -> Result<Option<T::Value<'c>>, CommandFailure>
    where
        VanillaArgument: Contains<T>,
        T: Argument<Source<'a, Self>>,
        ArgumentError: From<T::Error>,
    {
        let Some(span) = call.span(arg.node()) else {
            return Ok(None);
        };
        let source = self.source(call);
        match self.tree.0.value(&call.line, span, arg, &source) {
            None => Err(CommandFailure::failed()),
            Some(Ok(value)) => Ok(Some(value)),
            Some(Err(error)) => Err(CommandFailure::syntax(&call.line, &error.map_into())),
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

/// Parses, suggests and runs commands in a dimension world, against the tree the host froze.
#[derive(Clone)]
pub(crate) struct DimCommandPlugin {
    pub tree: CommandTree,
}

impl Plugin for DimCommandPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.tree.clone());
        app.init_resource::<CommandHandlers>();
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
