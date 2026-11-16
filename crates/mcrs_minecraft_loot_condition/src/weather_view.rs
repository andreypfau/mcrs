use mcrs_minecraft_predicate::NoWorld;

/// The weather of the world of an evaluation, as `weather_check` reads it (LOOT-CND-014).
/// `None` means that the host does not have the value.
pub trait WeatherView {
    /// The rain level, from 0 to 1. `None` also for a world that has no weather; the condition
    /// then reads a world that neither rains nor thunders.
    fn rain_strength(&self) -> Option<f32>;

    /// The thunder level, from 0 to 1. `None` also for a world that has no weather.
    fn thunder_strength(&self) -> Option<f32>;
}

impl WeatherView for NoWorld {
    fn rain_strength(&self) -> Option<f32> {
        None
    }

    fn thunder_strength(&self) -> Option<f32> {
        None
    }
}
