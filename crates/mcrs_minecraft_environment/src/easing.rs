use std::f32::consts::PI;
use std::fmt;

use mcrs_minecraft_nbt::tag::NbtTag;
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

/// A named easing curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamedEasing {
    Constant,
    Linear,
    InQuad,
    OutQuad,
    InOutQuad,
    InCubic,
    OutCubic,
    InOutCubic,
    InQuart,
    OutQuart,
    InOutQuart,
    InQuint,
    OutQuint,
    InOutQuint,
    InSine,
    OutSine,
    InOutSine,
    InExpo,
    OutExpo,
    InOutExpo,
    InCirc,
    OutCirc,
    InOutCirc,
    InBack,
    OutBack,
    InOutBack,
    InElastic,
    OutElastic,
    InOutElastic,
    InBounce,
    OutBounce,
    InOutBounce,
}

impl NamedEasing {
    pub const ALL: [NamedEasing; 32] = [
        NamedEasing::Constant,
        NamedEasing::Linear,
        NamedEasing::InQuad,
        NamedEasing::OutQuad,
        NamedEasing::InOutQuad,
        NamedEasing::InCubic,
        NamedEasing::OutCubic,
        NamedEasing::InOutCubic,
        NamedEasing::InQuart,
        NamedEasing::OutQuart,
        NamedEasing::InOutQuart,
        NamedEasing::InQuint,
        NamedEasing::OutQuint,
        NamedEasing::InOutQuint,
        NamedEasing::InSine,
        NamedEasing::OutSine,
        NamedEasing::InOutSine,
        NamedEasing::InExpo,
        NamedEasing::OutExpo,
        NamedEasing::InOutExpo,
        NamedEasing::InCirc,
        NamedEasing::OutCirc,
        NamedEasing::InOutCirc,
        NamedEasing::InBack,
        NamedEasing::OutBack,
        NamedEasing::InOutBack,
        NamedEasing::InElastic,
        NamedEasing::OutElastic,
        NamedEasing::InOutElastic,
        NamedEasing::InBounce,
        NamedEasing::OutBounce,
        NamedEasing::InOutBounce,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        use NamedEasing::*;
        match self {
            Constant => "constant",
            Linear => "linear",
            InQuad => "in_quad",
            OutQuad => "out_quad",
            InOutQuad => "in_out_quad",
            InCubic => "in_cubic",
            OutCubic => "out_cubic",
            InOutCubic => "in_out_cubic",
            InQuart => "in_quart",
            OutQuart => "out_quart",
            InOutQuart => "in_out_quart",
            InQuint => "in_quint",
            OutQuint => "out_quint",
            InOutQuint => "in_out_quint",
            InSine => "in_sine",
            OutSine => "out_sine",
            InOutSine => "in_out_sine",
            InExpo => "in_expo",
            OutExpo => "out_expo",
            InOutExpo => "in_out_expo",
            InCirc => "in_circ",
            OutCirc => "out_circ",
            InOutCirc => "in_out_circ",
            InBack => "in_back",
            OutBack => "out_back",
            InOutBack => "in_out_back",
            InElastic => "in_elastic",
            OutElastic => "out_elastic",
            InOutElastic => "in_out_elastic",
            InBounce => "in_bounce",
            OutBounce => "out_bounce",
            InOutBounce => "in_out_bounce",
        }
    }

    #[must_use]
    pub fn ease(self, x: f32) -> f32 {
        use NamedEasing::*;
        const C1: f32 = 1.70158;
        const C2: f32 = 2.5949094;
        const C3: f32 = 2.70158;
        const C4: f32 = 2.0 * PI / 3.0;
        const C5: f32 = 4.0 * PI / 9.0;
        let half = x < 0.5;
        match self {
            Constant => 0.0,
            Linear => x,
            InQuad => x.powi(2),
            InCubic => x.powi(3),
            InQuart => x.powi(4),
            InQuint => x.powi(5),
            OutQuad => 1.0 - (1.0 - x).powi(2),
            OutCubic => 1.0 - (1.0 - x).powi(3),
            OutQuart => 1.0 - (1.0 - x).powi(4),
            OutQuint => 1.0 - (1.0 - x).powi(5),
            InOutQuad => in_out_power(x, 2),
            InOutCubic => in_out_power(x, 3),
            InOutQuart => in_out_power(x, 4),
            InOutQuint => in_out_power(x, 5),
            InSine => 1.0 - (PI * x / 2.0).cos(),
            OutSine => (PI * x / 2.0).sin(),
            InOutSine => (1.0 - (PI * x).cos()) / 2.0,
            InExpo if x == 0.0 => 0.0,
            InExpo => 2f32.powf(10.0 * x - 10.0),
            OutExpo if x == 1.0 => 1.0,
            OutExpo => 1.0 - 2f32.powf(-10.0 * x),
            InOutExpo if half && x == 0.0 => 0.0,
            InOutExpo if half => 2f32.powf(20.0 * x - 10.0) / 2.0,
            InOutExpo if x == 1.0 => 1.0,
            InOutExpo => (2.0 - 2f32.powf(10.0 - 20.0 * x)) / 2.0,
            InCirc => 1.0 - (1.0 - x * x).sqrt(),
            OutCirc => (1.0 - (x - 1.0).powi(2)).sqrt(),
            InOutCirc if half => (1.0 - (1.0 - (2.0 * x).powi(2)).sqrt()) / 2.0,
            InOutCirc => ((1.0 - (2.0 - 2.0 * x).powi(2)).sqrt() + 1.0) / 2.0,
            InBack => C3 * x.powi(3) - C1 * x.powi(2),
            OutBack => 1.0 + C3 * (x - 1.0).powi(3) + C1 * (x - 1.0).powi(2),
            InOutBack if half => (2.0 * x).powi(2) * ((C2 + 1.0) * 2.0 * x - C2) / 2.0,
            InOutBack => {
                ((2.0 * x - 2.0).powi(2) * ((C2 + 1.0) * (2.0 * x - 2.0) + C2) + 2.0) / 2.0
            }
            InElastic if x == 0.0 => 0.0,
            InElastic if x == 1.0 => 1.0,
            InElastic => -2f32.powf(10.0 * x - 10.0) * ((10.0 * x - 10.75) * C4).sin(),
            OutElastic if x == 0.0 => 0.0,
            OutElastic if x == 1.0 => 1.0,
            OutElastic => 2f32.powf(-10.0 * x) * ((10.0 * x - 0.75) * C4).sin() + 1.0,
            InOutElastic if half && x == 0.0 => 0.0,
            InOutElastic if half => {
                -(2f32.powf(20.0 * x - 10.0) * ((20.0 * x - 11.125) * C5).sin()) / 2.0
            }
            InOutElastic if x == 1.0 => 1.0,
            InOutElastic => {
                2f32.powf(10.0 - 20.0 * x) * ((20.0 * x - 11.125) * C5).sin() / 2.0 + 1.0
            }
            OutBounce => out_bounce(x),
            InBounce => 1.0 - out_bounce(1.0 - x),
            InOutBounce if half => (1.0 - out_bounce(1.0 - 2.0 * x)) / 2.0,
            InOutBounce => (1.0 + out_bounce(2.0 * x - 1.0)) / 2.0,
        }
    }
}

fn in_out_power(x: f32, power: i32) -> f32 {
    if x < 0.5 {
        2f32.powi(power - 1) * x.powi(power)
    } else {
        1.0 - (2.0 - 2.0 * x).powi(power) / 2.0
    }
}

fn out_bounce(x: f32) -> f32 {
    const N: f32 = 7.5625;
    const D: f32 = 2.75;
    if x < 1.0 / D {
        N * x * x
    } else if x < 2.0 / D {
        let x = x - 1.5 / D;
        N * x * x + 0.75
    } else if x < 2.5 / D {
        let x = x - 2.25 / D;
        N * x * x + 0.9375
    } else {
        let x = x - 2.625 / D;
        N * x * x + 0.984375
    }
}

/// The easing of a track: a named curve or a cubic Bézier curve through `(0, 0)` and `(1, 1)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Named(NamedEasing),
    CubicBezier([f32; 4]),
}

impl Default for Easing {
    fn default() -> Self {
        Easing::Named(NamedEasing::Linear)
    }
}

fn cubic(a: f32, b: f32, c: f32, u: f32) -> f32 {
    ((a * u + b) * u + c) * u
}

fn bezier_coefficients(p1: f32, p2: f32) -> (f32, f32, f32) {
    (3.0 * p1 - 3.0 * p2 + 1.0, -6.0 * p1 + 3.0 * p2, 3.0 * p1)
}

impl Easing {
    #[must_use]
    pub fn ease(self, x: f32) -> f32 {
        match self {
            Easing::Named(named) => named.ease(x),
            Easing::CubicBezier([x1, y1, x2, y2]) => {
                let (ax, bx, cx) = bezier_coefficients(x1, x2);
                let (ay, by, cy) = bezier_coefficients(y1, y2);
                // With x1 and x2 in [0, 1] the curve's X rises over [0, 1], so a bisection
                // finds the parameter.
                let (mut low, mut high) = (0.0f32, 1.0f32);
                let mut u = 0.5;
                for _ in 0..32 {
                    if cubic(ax, bx, cx, u) < x {
                        low = u;
                    } else {
                        high = u;
                    }
                    u = (low + high) / 2.0;
                }
                cubic(ay, by, cy, u)
            }
        }
    }

    pub(crate) fn read(tag: &NbtTag) -> Result<Easing, String> {
        match tag {
            NbtTag::String(name) => NamedEasing::ALL
                .into_iter()
                .find(|easing| easing.name() == name)
                .map(Easing::Named)
                .ok_or_else(|| format!("unknown easing `{name}`")),
            NbtTag::Compound(fields) => {
                let Some(NbtTag::List(points)) = fields.get("cubic_bezier") else {
                    return Err("expected an easing name or `cubic_bezier`".to_owned());
                };
                let [x1, y1, x2, y2] = points.as_slice() else {
                    return Err("`cubic_bezier` holds four numbers".to_owned());
                };
                let number = crate::read_float;
                let curve = [number(x1)?, number(y1)?, number(x2)?, number(y2)?];
                if !(0.0..=1.0).contains(&curve[0]) || !(0.0..=1.0).contains(&curve[2]) {
                    return Err("the x of a cubic Bézier control point is not in [0; 1]".to_owned());
                }
                Ok(Easing::CubicBezier(curve))
            }
            _ => Err("expected an easing name or `cubic_bezier`".to_owned()),
        }
    }
}

impl fmt::Display for NamedEasing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl Serialize for Easing {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Easing::Named(named) => serializer.serialize_str(named.name()),
            Easing::CubicBezier(curve) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("cubic_bezier", curve)?;
                map.end()
            }
        }
    }
}
