//! The command engine: the command tree, the registration of commands and their handlers, and
//! the pass that parses and runs the command lines of the players across every dimension world.
//! It registers no command and knows no argument type: the application composes a
//! [`CommandSet`].

// std: bevy_ecs needs it.
mod answer_suggestion;
mod app_commands;
mod call;
mod command_event;
mod command_failure;
mod command_limits;
mod command_outbox;
mod command_output;
mod command_queue;
mod command_set;
mod command_targets;
mod command_tree;
mod context;
mod dimension_entry;
mod effect_queue;
mod executor;
mod handlers;
mod invocation;
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
pub use command_set::{CommandSet, SetLookup};
pub use command_targets::CommandTargets;
pub use command_tree::{CommandTree, CommandTreePackets, SetGraph};
pub use context::CommandContext;
pub use dimension_entry::DimensionEntry;
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
