use bevy_ecs::bundle::Bundle;

#[derive(Bundle, Default)]
pub struct LivingAttributesBundle {}

pub trait Attribute: Default {
    fn base_value(&self) -> f32;

    fn value(&self) -> f32 {
        let base = self.base_value();
        Self::sanitize_value(base)
    }

    fn sanitize_value(value: f32) -> f32 {
        value
    }
}

pub trait RangedAttribute: Attribute {
    fn min_value() -> f32;
    fn max_value() -> f32;

    fn sanitize_value(value: f32) -> f32 {
        if value.is_nan() {
            Self::min_value()
        } else {
            value.clamp(Self::min_value(), Self::max_value())
        }
    }
}
