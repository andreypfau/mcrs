use bevy_ecs::system::{ReadOnlySystemParam, SystemParamItem};
use mcrs_command::ArgumentSet;
use mcrs_minecraft_command::{ErrorText, Source, WireArgument};

pub trait CommandSet: Send + Sync + 'static {
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
    type Error: ErrorText + Copy + Send + Sync + 'static;
    type Lookup: ReadOnlySystemParam + 'static;
}

pub type SetLookup<'w, 's, S> = SystemParamItem<'w, 's, <S as CommandSet>::Lookup>;
