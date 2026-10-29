use crate::worlds::Worlds;
use bevy_app::{App, Plugin};
use bevy_ecs::resource::Resource;
use std::collections::VecDeque;

/// Applies the oldest effect of one type that a handler wrote in the world at the index.
pub(crate) type ApplyNext = fn(&mut Worlds<'_, '_>, usize);

/// The effects of type `E` that a handler of this world wrote and the host did not apply yet,
/// in the order of writing, and the function that applies one. A handler writes one with
/// [`CommandOutput::effect`](crate::CommandOutput::effect).
#[derive(Resource)]
pub struct EffectQueue<E: Send + Sync + 'static> {
    pub(crate) values: VecDeque<E>,
    apply: fn(&mut Worlds<'_, '_>, E),
}

pub(crate) fn apply_next<E: Send + Sync + 'static>(worlds: &mut Worlds<'_, '_>, index: usize) {
    let next = worlds
        .world_mut(index)
        .and_then(|world| world.get_resource_mut::<EffectQueue<E>>())
        .and_then(|mut queue| Some((queue.apply, queue.values.pop_front()?)));
    match next {
        Some((apply, effect)) => apply(worlds, effect),
        None => tracing::error!("a command effect is not in the queue of its world"),
    }
}

/// Adds the queue of the effects of type `E` to each dimension world.
pub(crate) struct EffectPlugin<E> {
    pub(crate) apply: fn(&mut Worlds<'_, '_>, E),
}

impl<E> Clone for EffectPlugin<E> {
    fn clone(&self) -> Self {
        Self { apply: self.apply }
    }
}

impl<E: Send + Sync + 'static> Plugin for EffectPlugin<E> {
    fn build(&self, app: &mut App) {
        app.insert_resource(EffectQueue {
            values: VecDeque::new(),
            apply: self.apply,
        });
    }

    fn is_unique(&self) -> bool {
        false
    }
}
