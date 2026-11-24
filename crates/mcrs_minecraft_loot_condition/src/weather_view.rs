use mcrs_minecraft_predicate::BlockHost;

pub trait WeatherView {
    fn rain_strength(&self) -> Option<f32>;

    fn thunder_strength(&self) -> Option<f32> {
        None
    }
}

impl<F, E> WeatherView for BlockHost<F, E> {
    fn rain_strength(&self) -> Option<f32> {
        None
    }
}
