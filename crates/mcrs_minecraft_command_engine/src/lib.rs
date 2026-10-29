//! The command engine: the command tree, the registration of commands and their handlers, and
//! the pass that parses and runs the command lines of the players across every dimension world.
//! It registers no command.

// std: bevy_ecs, mcrs_minecraft_core and mcrs_minecraft_protocol, which hold the worlds, the
// identifiers and the command packets, need it.
mod answer_suggestion;
mod app_commands;
mod call;
mod command_event;
mod command_failure;
mod command_limits;
mod command_outbox;
mod command_output;
mod command_queue;
mod command_targets;
mod command_tree;
mod context;
mod dimension_entry;
mod dimension_order;
mod effect_queue;
mod executor;
mod handlers;
mod invocation;
mod lookup;
mod modified;
mod operator_broadcast;
mod parse_line;
mod pass;
mod player_entry;
mod plugin;
mod registrar;
mod running_command;
mod source_state;
mod system_message;
mod worlds;

pub use app_commands::{AppCommandsExt, RegistrationError};
pub use call::Call;
pub use command_event::CommandEvent;
pub use command_failure::{CommandFailure, CommandResult};
pub use command_limits::CommandLimits;
pub use command_output::{CommandOutput, Feedback};
pub use command_targets::CommandTargets;
pub use command_tree::{CommandTree, CommandTreePackets};
pub use context::CommandContext;
pub use dimension_entry::DimensionEntry;
pub use dimension_order::{DimensionWorld, in_dimension_order};
pub use effect_queue::EffectQueue;
pub use invocation::Invocation;
pub use modified::{Modified, ModifyResult};
pub use operator_broadcast::OperatorBroadcast;
pub use pass::run_pending_commands;
pub use player_entry::PlayerEntry;
pub use plugin::CommandPlugin;
pub use registrar::{CommandRegistrar, Handler, NodeRef};
pub use source_state::{SourceState, level_of};
pub use system_message::{system_chat, system_message};
pub use worlds::{PlayerChangeError, Worlds};
