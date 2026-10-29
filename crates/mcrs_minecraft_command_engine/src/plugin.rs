use crate::command_limits::CommandLimits;
use crate::command_outbox::CommandOutbox;
use crate::command_queue::{CommandQueue, queue_command_line, queue_suggestion_request};
use crate::command_targets::CommandTargets;
use crate::command_tree::{CommandTree, CommandTreeBuilder, CommandTreePackets};
use crate::handlers::{CommandHandlers, ModifierHandlers};
use crate::pass::PassBuffers;
use crate::running_command::RunningCommand;
use bevy_app::{App, Plugin};
use mcrs_command::GraphBuilder;
use mcrs_minecraft_level::world::sub_app::AddDimensionPlugins;

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
        app.init_resource::<PassBuffers>();
        app.init_resource::<CommandQueue>();
        app.add_observer(queue_command_line)
            .add_observer(queue_suggestion_request);
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

/// Holds the handlers the host runs in a dimension world, the resources the host lends to it,
/// and the `commands` packets the world sends.
#[derive(Clone)]
struct DimCommandPlugin {
    packets: CommandTreePackets,
}

impl Plugin for DimCommandPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.packets.clone());
        app.init_resource::<CommandTree>();
        app.init_resource::<CommandHandlers>();
        app.init_resource::<ModifierHandlers>();
        app.init_resource::<RunningCommand>();
        app.init_resource::<CommandTargets>();
        app.init_resource::<CommandOutbox>();
    }
}
