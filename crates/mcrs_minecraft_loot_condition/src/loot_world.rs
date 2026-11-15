use mcrs_minecraft_predicate::{EntityView, LocationView};

use crate::environment_view::EnvironmentView;
use crate::score_view::ScoreView;
use crate::storage_view::StorageView;

/// Each view of the world that a condition or a provider reads. `E` is the entity handle of the
/// host. Implement the views; this trait follows.
pub trait LootWorld<E>:
    LocationView + EntityView<Entity = E> + ScoreView<E> + StorageView + EnvironmentView
{
}

impl<E, W> LootWorld<E> for W where
    W: LocationView
        + EntityView<Entity = E>
        + ScoreView<E>
        + StorageView
        + EnvironmentView
        + ?Sized
{
}
