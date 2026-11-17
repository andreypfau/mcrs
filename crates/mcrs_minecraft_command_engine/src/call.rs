use crate::invocation::Invocation;
use bevy_ecs::prelude::In;

#[derive(Clone)]
pub struct Call<B, C> {
    pub branch: B,
    pub command: C,
    pub invocation: Invocation,
}

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
