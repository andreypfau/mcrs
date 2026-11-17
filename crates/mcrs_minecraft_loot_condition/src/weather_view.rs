use mcrs_minecraft_predicate::NoWorld;

pub trait WeatherView {
    fn rain_strength(&self) -> Option<f32>;

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
