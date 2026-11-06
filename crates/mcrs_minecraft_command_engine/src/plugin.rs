use crate::command_limits::CommandLimits;
use crate::command_outbox::CommandOutbox;
use crate::command_queue::{
    CommandQueue, queue_command_line, queue_suggestion_request, reject_illegal_command_line,
};
use crate::command_set::CommandSet;
use crate::command_targets::CommandTargets;
use crate::command_tree::{
    CommandTree, CommandTreeBuilder, CommandTreePackets, SetGraph, send_command_tree,
};
use crate::handlers::{CommandHandlers, ModifierHandlers};
use crate::pass::{PassBuffers, commands_pending, run_commands};
use crate::running_command::RunningCommand;
use bevy_app::{App, FixedPreUpdate, Plugin, Update};
use bevy_ecs::schedule::IntoScheduleConfigs;
use mcrs_command::GraphBuilder;
use mcrs_minecraft_engine::ability::{AnswerOpLevel, OpLevelAnnounced};
use mcrs_minecraft_engine::command_handoff::CommandHandoff;
use mcrs_minecraft_engine::dim_tick::DimInboxDrain;
use mcrs_minecraft_engine::drain_player_despawn::drain_inbound_player_despawn;
use mcrs_minecraft_engine::minecraft_entity::InboundDispatch;
use mcrs_minecraft_engine_world::entity::add_entity_to_chunk;
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use std::marker::PhantomData;

/// Owns the command tree of the set `S`: plugins register commands while the app builds, and
/// the tree freezes when it finishes. Every dimension world gets a copy of the tree and runs the
/// command lines of its players in its own tick.
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
        app.add_observer(reject_illegal_command_line);
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
        let limits = app
            .world()
            .get_resource::<CommandLimits>()
            .copied()
            .unwrap_or_default();
        match CommandTreePackets::new::<S>(&graph) {
            Ok(packets) => {
                app.insert_resource(CommandTree::<S> {
                    graph: graph.clone(),
                });
                app.add_dimension_plugin(DimCommandPlugin::<S> {
                    packets,
                    graph,
                    limits,
                });
            }
            Err(error) => tracing::error!("the command tree does not encode: {error:#}"),
        }
    }
}

/// The command tree, the handlers and the command run of one dimension world, and the
/// `commands` packets the world sends.
struct DimCommandPlugin<S: CommandSet> {
    packets: CommandTreePackets,
    graph: SetGraph<S>,
    limits: CommandLimits,
}

impl<S: CommandSet> Clone for DimCommandPlugin<S> {
    fn clone(&self) -> Self {
        Self {
            packets: self.packets.clone(),
            graph: self.graph.clone(),
            limits: self.limits,
        }
    }
}

impl<S: CommandSet> Plugin for DimCommandPlugin<S> {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.packets.clone());
        app.insert_resource(CommandTree::<S> {
            graph: self.graph.clone(),
        });
        app.insert_resource(self.limits);
        app.init_resource::<CommandHandlers>();
        app.init_resource::<ModifierHandlers>();
        app.init_resource::<RunningCommand>();
        app.init_resource::<CommandTargets>();
        app.init_resource::<CommandOutbox>();
        app.init_resource::<CommandQueue>();
        app.init_resource::<PassBuffers>();
        app.add_message::<OpLevelAnnounced>();
        app.add_message::<CommandHandoff>();
        app.add_observer(queue_command_line)
            .add_observer(queue_suggestion_request);
        app.add_systems(Update, send_command_tree.in_set(AnswerOpLevel));
        app.add_systems(
            FixedPreUpdate,
            run_commands::<S>
                .run_if(commands_pending)
                .after(DimInboxDrain)
                .after(InboundDispatch)
                .after(drain_inbound_player_despawn)
                .before(add_entity_to_chunk),
        );
    }
}
