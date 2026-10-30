use crate::command_failure::CommandResult;
use crate::invocation::Invocation;
use crate::modified::ModifyResult;
use bevy_ecs::prelude::In;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::SystemId;

/// The command a node runs: the dimension worlds resolve it to their own handler system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct HandlerKey(pub(crate) u16);

/// The handlers of one output type, indexed by [`HandlerKey`].
#[derive(Resource)]
pub(crate) struct Handlers<O: Send + Sync + 'static>(
    pub(crate) Vec<Option<Vec<Option<SystemId<In<Invocation>, O>>>>>,
);

impl<O: Send + Sync + 'static> Default for Handlers<O> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<O: Send + Sync + 'static> Handlers<O> {
    /// Sets the systems of `key`, one per branch in the order of the branch index. A branch
    /// that the table does not serve has none.
    pub(crate) fn insert(
        &mut self,
        key: HandlerKey,
        branches: Vec<Option<SystemId<In<Invocation>, O>>>,
    ) {
        let index = usize::from(key.0);
        if self.0.len() <= index {
            self.0.resize_with(index + 1, || None);
        }
        if let Some(slot) = self.0.get_mut(index) {
            *slot = Some(branches);
        }
    }

    /// `None` when the key has no handler or `branch` is outside its table.
    pub(crate) fn get(&self, key: HandlerKey, branch: u16) -> Option<SystemId<In<Invocation>, O>> {
        let branches = self.0.get(usize::from(key.0))?.as_ref()?;
        branches.get(usize::from(branch)).copied().flatten()
    }
}

pub(crate) type CommandHandlers = Handlers<CommandResult>;
pub(crate) type ModifierHandlers = Handlers<ModifyResult>;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    #[test]
    fn only_a_served_branch_of_an_inserted_key_has_a_handler() {
        let mut world = World::new();
        let system = world.register_system(|_: In<Invocation>| Ok(0));
        let mut handlers = CommandHandlers::default();
        handlers.insert(HandlerKey(1), vec![Some(system), None]);
        for (key, branch, found) in [
            (1, 0, true),
            (1, 1, false),
            (1, 2, false),
            (0, 0, false),
            (9, 0, false),
        ] {
            assert_eq!(
                handlers.get(HandlerKey(key), branch).is_some(),
                found,
                "key {key} branch {branch}"
            );
        }
    }
}
