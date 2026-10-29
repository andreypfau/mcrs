use bevy_ecs::system::{ReadOnlySystemParam, SystemParamItem};
use mcrs_command::ArgumentSet;
use mcrs_minecraft_command::{ErrorText, Source, WireArgument};

/// The arguments the command tree parses and what they read from a dimension world. An
/// application composes one and names it on [`CommandPlugin`](crate::CommandPlugin), on
/// [`run_pending_commands`](crate::run_pending_commands) and on the registrar of each command.
pub trait CommandSet: Send + Sync + 'static {
    /// The argument type of an argument node.
    type Argument: WireArgument
        + Clone
        + PartialEq
        + Send
        + Sync
        + 'static
        + for<'a, 'w, 's> ArgumentSet<
            Source<'a, SetLookup<'w, 's, Self>>,
            Error = Self::Error,
            Tooltip = &'static str,
        >;
    /// The errors of every argument type of the set.
    type Error: ErrorText + Copy + Send + Sync + 'static;
    /// What the arguments read from the world a line parses in: the lookup of the source.
    type Lookup: ReadOnlySystemParam + 'static;
}

/// The lookup of a source of the set `S` in a dimension world.
pub type SetLookup<'w, 's, S> = SystemParamItem<'w, 's, <S as CommandSet>::Lookup>;
