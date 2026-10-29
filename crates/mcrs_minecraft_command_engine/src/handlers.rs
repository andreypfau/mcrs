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
pub(crate) struct Handlers<O: Send + Sync + 'static>(pub(crate) Vec<Option<HandlerSlot<O>>>);

/// A handler system and the number of branches its table holds.
pub(crate) struct HandlerSlot<O: Send + Sync + 'static> {
    pub(crate) system: SystemId<In<Invocation>, O>,
    pub(crate) branches: u16,
}

impl<O: Send + Sync + 'static> Default for Handlers<O> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<O: Send + Sync + 'static> Handlers<O> {
    /// `None` when the key has no handler or `branch` is outside its table.
    pub(crate) fn get(&self, key: HandlerKey, branch: u16) -> Option<SystemId<In<Invocation>, O>> {
        let slot = self.0.get(usize::from(key.0))?.as_ref()?;
        (branch < slot.branches).then_some(slot.system)
    }
}

pub(crate) type CommandHandlers = Handlers<CommandResult>;
pub(crate) type ModifierHandlers = Handlers<ModifyResult>;
