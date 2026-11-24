use mcrs_minecraft_predicate::NoWorld;

pub trait WeatherView {
    fn rain_strength(&self) -> Option<f32> {
        None
    }

    fn thunder_strength(&self) -> Option<f32> {
        None
    }
}

impl WeatherView for NoWorld {}
