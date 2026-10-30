use crate::invocation::Invocation;
use bevy_ecs::prelude::In;

/// The command line a handler runs, the branch its executable node names, and the handles
/// the command's builder returned.
#[derive(Clone)]
pub struct Call<B, C> {
    /// The branch the executable node of the call names.
    pub branch: B,
    /// The handles that the builder of the command returned.
    pub command: C,
    /// The run of the stage that the call belongs to.
    pub invocation: Invocation,
}

/// Each registration owns its branch and handles, so commands that share `B` or `C` stay apart.
pub(crate) fn to_call<B: Copy, C: Copy>(
    branch: B,
    command: C,
) -> impl FnMut(In<Invocation>) -> Call<B, C> + Clone {
    move |In(invocation)| Call {
        branch,
        command,
        invocation,
    }
}
