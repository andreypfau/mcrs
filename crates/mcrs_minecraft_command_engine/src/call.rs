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

#[derive(Clone)]
pub(crate) struct BranchTable<B> {
    pub(crate) first: B,
    pub(crate) rest: Vec<B>,
}

impl<B: Copy> BranchTable<B> {
    pub(crate) fn len(&self) -> u16 {
        u16::try_from(self.rest.len())
            .ok()
            .and_then(|rest| rest.checked_add(1))
            .unwrap_or(u16::MAX)
    }

    /// The handler registry refuses an index outside the table before the call runs, so the fallback to the first branch is not reached.
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
