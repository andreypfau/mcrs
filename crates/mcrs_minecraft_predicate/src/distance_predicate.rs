use glam::DVec3;
use mcrs_minecraft_value_provider::bounds::MinMaxBounds;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub struct DistancePredicate {
    #[serde(default)]
    x: MinMaxBounds<f64>,
    #[serde(default)]
    y: MinMaxBounds<f64>,
    #[serde(default)]
    z: MinMaxBounds<f64>,
    #[serde(default)]
    horizontal: MinMaxBounds<f64>,
    #[serde(default)]
    absolute: MinMaxBounds<f64>,
}

impl DistancePredicate {
    #[must_use]
    pub fn matches(&self, a: DVec3, b: DVec3) -> bool {
        let x = (a.x - b.x) as f32;
        let y = (a.y - b.y) as f32;
        let z = (a.z - b.z) as f32;
        let horizontal = x * x + z * z;
        let absolute = x * x + y * y + z * z;
        self.x.matches(f64::from(x.abs()))
            && self.y.matches(f64::from(y.abs()))
            && self.z.matches(f64::from(z.abs()))
            && matches_squared(&self.horizontal, f64::from(horizontal))
            && matches_squared(&self.absolute, f64::from(absolute))
    }
}

pub(crate) fn matches_squared(bounds: &MinMaxBounds<f64>, squared: f64) -> bool {
    bounds.min.is_none_or(|min| min * min <= squared)
        && bounds.max.is_none_or(|max| squared <= max * max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_squared_form_squares_each_bound_pred_num_003() {
        let bounds = MinMaxBounds {
            min: Some(-2.0),
            max: Some(3.0),
        };
        let cases: [(f64, bool); 4] = [(4.0, true), (3.99, false), (9.0, true), (9.01, false)];
        for (squared, expected) in cases {
            assert_eq!(matches_squared(&bounds, squared), expected, "{squared}");
        }
    }
}
