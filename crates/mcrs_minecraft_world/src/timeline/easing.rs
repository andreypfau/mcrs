use serde::de::{Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::TrackError;

/// `EasingType`: the curve a segment's alpha is bent through.
///
/// The reference registers about thirty more named curves. Only the three the
/// shipped timelines use are implemented; every other name is an error rather
/// than a silent fallback to linear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    Constant,
    CubicBezier(CubicBezier),
}

impl Easing {
    pub fn apply(self, x: f32) -> f32 {
        match self {
            Easing::Linear => x,
            Easing::Constant => 0.0,
            Easing::CubicBezier(bezier) => bezier.apply(x),
        }
    }
}

impl<'de> Deserialize<'de> for Easing {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(EasingVisitor)
    }
}

struct EasingVisitor;

impl<'de> Visitor<'de> for EasingVisitor {
    type Value = Easing;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("an easing name or {\"cubic_bezier\": [x1, y1, x2, y2]}")
    }

    fn visit_str<E: serde::de::Error>(self, name: &str) -> Result<Easing, E> {
        match name {
            "linear" => Ok(Easing::Linear),
            "constant" => Ok(Easing::Constant),
            _ => Err(E::custom(TrackError::UnsupportedEasing(name.to_owned()))),
        }
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Easing, A::Error> {
        let Some(name) = map.next_key::<String>()? else {
            return Err(A::Error::custom("`ease` object names no curve"));
        };
        if name != "cubic_bezier" {
            return Err(A::Error::custom(TrackError::UnsupportedEasing(name)));
        }
        let [x1, y1, x2, y2] = map.next_value::<[f32; 4]>()?;
        let easing = CubicBezier::new(x1, y1, x2, y2)
            .map(Easing::CubicBezier)
            .map_err(A::Error::custom)?;
        if let Some(extra) = map.next_key::<String>()? {
            return Err(A::Error::custom(format!(
                "`ease` object names more than one curve, including `{extra}`"
            )));
        }
        Ok(easing)
    }
}

impl Serialize for Easing {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Easing::Linear => s.serialize_str("linear"),
            Easing::Constant => s.serialize_str("constant"),
            Easing::CubicBezier(bezier) => {
                let mut map = s.serialize_map(Some(1))?;
                map.serialize_entry("cubic_bezier", &bezier.controls)?;
                map.end()
            }
        }
    }
}

/// `EasingType.CubicBezier`, with its two cubics derived from the control
/// points once here rather than on every sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    controls: [f32; 4],
    x: CubicCurve,
    y: CubicCurve,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CubicCurve {
    a: f32,
    b: f32,
    c: f32,
}

impl CubicCurve {
    fn from_controls(v1: f32, v2: f32) -> Self {
        CubicCurve {
            a: 3.0 * v1 - 3.0 * v2 + 1.0,
            b: -6.0 * v1 + 3.0 * v2,
            c: 3.0 * v1,
        }
    }

    fn sample(self, t: f32) -> f32 {
        ((self.a * t + self.b) * t + self.c) * t
    }

    fn gradient(self, t: f32) -> f32 {
        (3.0 * self.a * t + 2.0 * self.b) * t + self.c
    }
}

impl CubicBezier {
    const NEWTON_RAPHSON_ITERATIONS: usize = 4;
    const MAX_STEP: f32 = 0.25;
    const EPSILON: f32 = 1.0e-5;

    /// Only the two x controls are constrained: a y outside `[0; 1]` overshoots,
    /// which is a legal curve, while an x outside it would not be a function.
    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Result<Self, TrackError> {
        for (name, value) in [("x1", x1), ("x2", x2)] {
            if !(0.0..=1.0).contains(&value) {
                return Err(TrackError::BezierControl { name, value });
            }
        }
        Ok(CubicBezier {
            controls: [x1, y1, x2, y2],
            x: CubicCurve::from_controls(x1, x2),
            y: CubicCurve::from_controls(y1, y2),
        })
    }

    pub fn apply(self, x: f32) -> f32 {
        self.y.sample(self.solve_t(x))
    }

    fn solve_t(self, x: f32) -> f32 {
        let mut t = x;
        for _ in 0..Self::NEWTON_RAPHSON_ITERATIONS {
            let error = self.x.sample(t) - x;
            if error.abs() < Self::EPSILON {
                return t;
            }
            let gradient = self.x.gradient(t);
            if gradient < Self::EPSILON {
                break;
            }
            t -= (error / gradient).clamp(-Self::MAX_STEP, Self::MAX_STEP);
        }
        self.solve_t_bisect(x, t)
    }

    fn solve_t_bisect(self, x: f32, initial_t: f32) -> f32 {
        let (mut low, mut high) = (0.0f32, 1.0f32);
        let mut t = initial_t;
        // The reference loops until the bracket closes. Halving a float bracket
        // can stall on adjacent floats, so the count is bounded as well; the
        // bracket is far smaller than EPSILON long before the bound is reached.
        for _ in 0..64 {
            if low >= high {
                break;
            }
            let error = self.x.sample(t) - x;
            if error.abs() < Self::EPSILON {
                return t;
            }
            if error < 0.0 {
                low = t;
            } else {
                high = t;
            }
            t = (high + low) / 2.0;
        }
        t
    }
}
