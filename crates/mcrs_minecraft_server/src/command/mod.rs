mod dim;
mod lookup;
mod run;
pub mod vanilla;

pub use dim::DebugCommandsPlugin;
pub use lookup::DimLookup;
pub use run::{CommandEvent, CommandFeedback};

use crate::world::entity::player::ability::PlayerOpLevel;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::system::{SystemId, SystemParam};
use bevy_math::DVec3;
use mcrs_command::{Arg, Argument, BuildError, GraphBuilder, NodeId, Span};
use mcrs_minecraft_command::{
    CommandLevel, CommandSyntaxError, IntoCommandSyntaxError, PermissionCheck, Source,
    VanillaArgument, VanillaGraph, error_message,
};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_protocol::text::{Color, IntoText, Text};
use mcrs_minecraft_registries::CurrentRegistries;
use std::marker::PhantomData;

/// The command a node runs: the dimension worlds resolve it to their own handler system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HandlerKey(u16);

pub type CommandGraph = VanillaGraph<HandlerKey>;

/// Each dimension world holds a copy.
#[derive(Resource, Clone, Debug)]
pub struct CommandTree(pub CommandGraph);

#[derive(Resource)]
struct CommandTreeBuilder {
    builder: GraphBuilder<VanillaArgument, PermissionCheck, HandlerKey>,
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
        app.add_observer(run::reject_illegal_characters);
    }

    fn finish(&self, app: &mut App) {
        let Some(tree) = app.world_mut().remove_resource::<CommandTreeBuilder>() else {
            return;
        };
        match tree.builder.freeze() {
            Ok(graph) => {
                app.insert_resource(CommandTree(graph));
            }
            Err(error) => {
                tracing::error!("the command tree does not freeze: {error}");
                if let Ok(empty) = GraphBuilder::new().freeze() {
                    app.insert_resource(CommandTree(empty));
                }
            }
        }
    }
}

/// Registers the nodes of one command. Each node it makes executable runs the handler of that
/// command.
pub struct CommandRegistrar<'a> {
    builder: &'a mut GraphBuilder<VanillaArgument, PermissionCheck, HandlerKey>,
    key: HandlerKey,
}

impl CommandRegistrar<'_> {
    pub const ROOT: NodeId = NodeId::ROOT;

    pub fn literal(
        &mut self,
        parent: NodeId,
        name: &str,
        check: PermissionCheck,
    ) -> Result<NodeId, BuildError> {
        self.builder.literal(parent, name, check)
    }

    pub fn argument<T: Into<VanillaArgument>>(
        &mut self,
        parent: NodeId,
        name: &str,
        argument: T,
        check: PermissionCheck,
    ) -> Result<Arg<T>, BuildError> {
        self.builder.argument(parent, name, argument, check)
    }

    pub fn executes(&mut self, node: NodeId) -> Result<(), BuildError> {
        self.builder.executes(node, self.key)
    }

    pub fn redirect(&mut self, node: NodeId, target: NodeId) -> Result<(), BuildError> {
        self.builder.redirect(node, target)
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

    pub fn syntax(line: &str, error: &CommandSyntaxError) -> Self {
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
    pub position: DVec3,
    pub yaw: f32,
    pub pitch: f32,
}

/// The command line a handler runs, with the nodes of the context that runs.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub player: Entity,
    pub source: SourceState,
    pub line: String,
    nodes: Vec<(NodeId, Span)>,
}

impl Invocation {
    pub fn has<T>(&self, arg: Arg<T>) -> bool {
        self.span(arg.node()).is_some()
    }

    fn span(&self, node: NodeId) -> Option<Span> {
        self.nodes
            .iter()
            .find(|&&(parsed, _)| parsed == node)
            .map(|&(_, span)| span)
    }

    pub fn source<'s, L>(&self, lookup: &'s L) -> Source<'s, L> {
        Source {
            position: self.source.position,
            yaw: self.source.yaw,
            pitch: self.source.pitch,
            eye_height: Some(mcrs_minecraft_entity::PLAYER_EYE_HEIGHT),
            ..Source::new(self.source.level, lookup)
        }
    }

    /// The value of an argument the command line holds, or `None` when the parse did not pass
    /// through it.
    pub fn get<'i, 's, T>(
        &'i self,
        tree: &CommandTree,
        arg: Arg<T>,
        source: &Source<'s, DimLookup<'s>>,
    ) -> Option<Result<T::Value<'i>, CommandFailure>>
    where
        VanillaArgument: mcrs_command::Contains<T>,
        T: Argument<Source<'s, DimLookup<'s>>>,
        mcrs_command::SyntaxError<T::Error>: IntoCommandSyntaxError,
    {
        let span = self.span(arg.node())?;
        let value = tree.0.value(&self.line, span, arg, source)?;
        Some(value.map_err(|error| CommandFailure::syntax(&self.line, &error.into_command_error())))
    }
}

/// What a handler reads its arguments with.
#[derive(SystemParam)]
pub struct CommandContext<'w, 's> {
    pub tree: Res<'w, CommandTree>,
    registries: Option<Res<'w, CurrentRegistries>>,
    profiles: Query<'w, 's, &'static GameProfile, With<Player>>,
}

impl CommandContext<'_, '_> {
    pub fn lookup(&self) -> DimLookup<'_> {
        self.lookup_with(Vec::new())
    }

    /// A lookup that fills `names` with the player names, reusing its allocation.
    pub fn lookup_with<'a>(&'a self, mut names: Vec<&'a str>) -> DimLookup<'a> {
        names.clear();
        names.extend(
            self.profiles
                .iter()
                .map(|profile| profile.username.as_str()),
        );
        DimLookup::new(
            self.registries.as_deref().map(|registries| &**registries),
            names,
        )
    }
}

#[derive(Resource, Default)]
struct CommandHandlers(Vec<Option<SystemId<In<Invocation>, CommandResult>>>);

impl CommandHandlers {
    fn get(&self, key: HandlerKey) -> Option<SystemId<In<Invocation>, CommandResult>> {
        self.0.get(usize::from(key.0)).copied().flatten()
    }
}

pub trait AppCommands {
    /// Registers a command: `build` adds its nodes and returns the handles its handler reads,
    /// and every dimension world runs `handler` when one of those nodes runs.
    fn add_command<C, S, M>(
        &mut self,
        build: impl FnOnce(&mut CommandRegistrar<'_>) -> Result<C, BuildError>,
        handler: S,
    ) -> &mut Self
    where
        C: Resource + Clone,
        S: IntoSystem<In<Invocation>, CommandResult, M> + Clone + Send + Sync + 'static,
        M: 'static;
}

impl AppCommands for App {
    fn add_command<C, S, M>(
        &mut self,
        build: impl FnOnce(&mut CommandRegistrar<'_>) -> Result<C, BuildError>,
        handler: S,
    ) -> &mut Self
    where
        C: Resource + Clone,
        S: IntoSystem<In<Invocation>, CommandResult, M> + Clone + Send + Sync + 'static,
        M: 'static,
    {
        let Some(mut tree) = self.world_mut().get_resource_mut::<CommandTreeBuilder>() else {
            tracing::error!(
                "{} is added before CommandPlugin or after the tree froze",
                std::any::type_name::<C>()
            );
            return self;
        };
        let key = HandlerKey(tree.next);
        tree.next += 1;
        let built = build(&mut CommandRegistrar {
            builder: &mut tree.builder,
            key,
        });
        match built {
            Ok(handles) => self.add_dimension_plugin(HandlerPlugin {
                handles,
                key,
                system: handler,
                marker: PhantomData::<fn() -> M>,
            }),
            Err(error) => {
                tracing::error!("{} does not register: {error}", std::any::type_name::<C>());
                self
            }
        }
    }
}

struct HandlerPlugin<C, S, M> {
    handles: C,
    key: HandlerKey,
    system: S,
    marker: PhantomData<fn() -> M>,
}

impl<C: Clone, S: Clone, M> Clone for HandlerPlugin<C, S, M> {
    fn clone(&self) -> Self {
        Self {
            handles: self.handles.clone(),
            key: self.key,
            system: self.system.clone(),
            marker: PhantomData,
        }
    }
}

impl<C, S, M> Plugin for HandlerPlugin<C, S, M>
where
    C: Resource + Clone,
    S: IntoSystem<In<Invocation>, CommandResult, M> + Clone + Send + Sync + 'static,
    M: 'static,
{
    fn build(&self, app: &mut App) {
        app.insert_resource(self.handles.clone());
        let id = app.world_mut().register_system(self.system.clone());
        let mut handlers = app.world_mut().get_resource_or_init::<CommandHandlers>();
        let index = usize::from(self.key.0);
        if handlers.0.len() <= index {
            handlers.0.resize(index + 1, None);
        }
        if let Some(slot) = handlers.0.get_mut(index) {
            *slot = Some(id);
        }
    }

    fn is_unique(&self) -> bool {
        false
    }
}

/// Parses, suggests and runs commands in a dimension world, against the tree the host froze.
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
    }
}

pub(crate) fn level_of(op_level: PlayerOpLevel) -> CommandLevel {
    CommandLevel::from_id(op_level.clamped())
}

pub(crate) use run::send_command_tree;
