use crate::command_limits::CommandLimits;
use crate::command_outbox::CommandOutbox;
use crate::command_queue::{CommandQueue, queue_command_line, queue_suggestion_request};
use crate::command_set::CommandSet;
use crate::command_targets::CommandTargets;
use crate::command_tree::{CommandTree, CommandTreeBuilder, CommandTreePackets, send_command_tree};
use crate::handlers::{CommandHandlers, ModifierHandlers};
use crate::pass::PassBuffers;
use crate::pass::run_pending_commands;
use crate::running_command::RunningCommand;
use bevy_app::{App, Plugin, Update};
use bevy_ecs::schedule::IntoScheduleConfigs;
use mcrs_command::GraphBuilder;
use mcrs_minecraft_engine::ability::{AnswerOpLevel, OpLevelAnnounced};
use mcrs_minecraft_engine::between_ticks::BetweenTicks;
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use std::marker::PhantomData;

/// Owns the command tree of the set `S`: plugins register commands while the app builds, and
/// the tree freezes when it finishes.
pub struct CommandPlugin<S>(PhantomData<fn() -> S>);

impl<S> CommandPlugin<S> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<S> Default for CommandPlugin<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: CommandSet> Plugin for CommandPlugin<S> {
    fn build(&self, app: &mut App) {
        app.insert_resource(CommandTreeBuilder::<S> {
            builder: GraphBuilder::new(),
            next: 0,
        });
        app.init_resource::<CommandLimits>();
        app.init_resource::<PassBuffers<S>>();
        app.init_resource::<CommandQueue>();
        app.add_observer(queue_command_line)
            .add_observer(queue_suggestion_request);
        app.world_mut()
            .get_resource_or_init::<BetweenTicks>()
            .add(run_pending_commands::<S>);
    }

    fn finish(&self, app: &mut App) {
        let Some(tree) = app.world_mut().remove_resource::<CommandTreeBuilder<S>>() else {
            return;
        };
        let graph = match tree.builder.freeze() {
            Ok(graph) => graph,
            Err(error) => {
                tracing::error!("the command tree does not freeze: {error}");
                return;
            }
        };
        match CommandTreePackets::new::<S>(&graph) {
            Ok(packets) => {
                app.insert_resource(CommandTree::<S> { graph: Some(graph) });
                app.add_dimension_plugin(DimCommandPlugin::<S> {
                    packets,
                    set: PhantomData,
                });
            }
            Err(error) => tracing::error!("the command tree does not encode: {error:#}"),
        }
    }
}

/// Holds the handlers the host runs in a dimension world, the resources the host lends to it,
/// and the `commands` packets the world sends.
struct DimCommandPlugin<S> {
    packets: CommandTreePackets,
    set: PhantomData<fn() -> S>,
}

impl<S> Clone for DimCommandPlugin<S> {
    fn clone(&self) -> Self {
        Self {
            packets: self.packets.clone(),
            set: PhantomData,
        }
    }
}

impl<S: CommandSet> Plugin for DimCommandPlugin<S> {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.packets.clone());
        app.init_resource::<CommandTree<S>>();
        app.init_resource::<CommandHandlers>();
        app.init_resource::<ModifierHandlers>();
        app.init_resource::<RunningCommand>();
        app.init_resource::<CommandTargets>();
        app.init_resource::<CommandOutbox>();
        app.add_message::<OpLevelAnnounced>();
        app.add_systems(Update, send_command_tree.in_set(AnswerOpLevel));
    }
}
