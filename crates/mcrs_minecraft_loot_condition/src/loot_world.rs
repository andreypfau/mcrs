use mcrs_minecraft_predicate::LocationView;

use crate::environment_view::EnvironmentView;
use crate::score_view::ScoreView;
use crate::storage_view::StorageView;

/// Each view of the world that a condition or a provider reads. `E` is the entity handle of the
/// host. Implement the views; this trait follows.
pub trait LootWorld<E>: LocationView + ScoreView<E> + StorageView + EnvironmentView {}

impl<E, W> LootWorld<E> for W where
    W: LocationView + ScoreView<E> + StorageView + EnvironmentView + ?Sized
{
}
