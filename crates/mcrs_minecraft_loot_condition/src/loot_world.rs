use mcrs_minecraft_predicate::{EntityView, LocationView};

use crate::clock_view::ClockView;
use crate::environment_view::EnvironmentView;
use crate::score_view::ScoreView;
use crate::storage_view::StorageView;
use crate::weather_view::WeatherView;

pub trait LootWorld<E>:
    LocationView
    + EntityView<Entity = E>
    + ScoreView<E>
    + StorageView
    + EnvironmentView
    + WeatherView
    + ClockView
{
}

impl<E, W> LootWorld<E> for W where
    W: LocationView
        + EntityView<Entity = E>
        + ScoreView<E>
        + StorageView
        + EnvironmentView
        + WeatherView
        + ClockView
        + ?Sized
{
}
