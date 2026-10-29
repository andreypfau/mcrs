use crate::invocation::Invocation;
use bevy_ecs::prelude::In;
use std::ops::Deref;

/// The command line a handler runs, the branch its executable node names, and the handles
/// the command's builder returned.
#[derive(Clone)]
pub struct Call<B, C> {
    /// The branch the executable node of the call names.
    pub branch: B,
    /// The handles that the builder of the command returned.
    pub command: C,
    invocation: Invocation,
}

impl<B, C> Deref for Call<B, C> {
    type Target = Invocation;

    fn deref(&self) -> &Invocation {
        &self.invocation
    }
}

#[derive(Clone)]
pub(crate) struct BranchTable<B> {
    pub(crate) first: B,
    pub(crate) rest: Vec<B>,
}

impl<B: Copy> BranchTable<B> {
    /// Every index a node holds came from this table, so the fallback is never taken.
    fn get(&self, index: u16) -> B {
        match index.checked_sub(1) {
            None => self.first,
            Some(rest) => self
                .rest
                .get(usize::from(rest))
                .copied()
                .unwrap_or(self.first),
        }
    }
}

/// Each registration owns its table and handles, so commands that share `B` or `C` stay apart.
pub(crate) fn to_call<B: Copy, C: Copy>(
    table: BranchTable<B>,
    command: C,
) -> impl FnMut(In<Invocation>) -> Call<B, C> + Clone {
    move |In(invocation)| Call {
        branch: table.get(invocation.branch),
        command,
        invocation,
    }
}
