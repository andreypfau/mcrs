use bevy_ecs::bundle::Bundle;

// The Bundle derive expands to a struct-update expression which clippy
// flags as needless on an empty bundle. Wrap in a module so the allow
// reaches the macro expansion site.
#[allow(clippy::needless_update)]
mod living_attributes_bundle {
    use super::Bundle;

    #[derive(Bundle, Default)]
    pub struct LivingAttributesBundle {}
}

pub use living_attributes_bundle::LivingAttributesBundle;

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
