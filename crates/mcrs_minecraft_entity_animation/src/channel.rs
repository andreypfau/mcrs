use mcrs_minecraft_entity_model::Json;
use mcrs_molang::Expression;

use crate::animation_diagnostic::{AnimationDiagnostic, DiagnosticKind};
use crate::field_compiler::FieldCompiler;

/// How the sample between a keyframe and the next one is computed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LerpMode {
    #[default]
    Linear,
    CatmullRom,
}

/// A keyframe: a time in seconds and the values on each side of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe {
    pub time: f32,
    pub pre: [Expression; 3],
    pub post: [Expression; 3],
    pub lerp_mode: LerpMode,
}

/// The keyframes of one channel, sorted by time.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub keyframes: Vec<Keyframe>,
}

/// The keyframes that a sample at one time uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    /// Before the first keyframe: its `pre`.
    Pre(usize),
    /// At or after the last keyframe: its `post`.
    Post(usize),
    /// Between the keyframe `from` and the next one.
    Linear {
        from: usize,
        alpha: f32,
    },
    CatmullRom {
        from: usize,
        alpha: f32,
    },
}

impl Segment {
    /// `query.key_frame_lerp_time` of the sample.
    #[must_use]
    pub const fn alpha(self) -> f32 {
        match self {
            Self::Pre(_) | Self::Post(_) => 0.0,
            Self::Linear { alpha, .. } | Self::CatmullRom { alpha, .. } => alpha,
        }
    }
}

impl Channel {
    /// The keyframes around `time`; `None` for a channel with no keyframe.
    #[must_use]
    pub fn segment(&self, time: f32) -> Option<Segment> {
        let after = self.keyframes.partition_point(|key| key.time <= time);
        let Some(from) = after.checked_sub(1) else {
            return (!self.keyframes.is_empty()).then_some(Segment::Pre(0));
        };
        let (Some(a), Some(b)) = (self.keyframes.get(from), self.keyframes.get(after)) else {
            return Some(Segment::Post(from));
        };
        let alpha = (time - a.time) / (b.time - a.time);
        let catmull_rom =
            a.lerp_mode == LerpMode::CatmullRom || b.lerp_mode == LerpMode::CatmullRom;
        Some(if catmull_rom {
            Segment::CatmullRom { from, alpha }
        } else {
            Segment::Linear { from, alpha }
        })
    }

    /// The value of the channel for `segment`. `axis` evaluates one axis expression; the
    /// sample evaluates only the expressions that it uses.
    pub fn sample(
        &self,
        segment: Segment,
        mut axis: impl FnMut(&Expression, usize) -> f32,
    ) -> [f32; 3] {
        let mut value = [0.0; 3];
        let keys = &self.keyframes;
        match segment {
            Segment::Pre(index) | Segment::Post(index) => {
                let Some(key) = keys.get(index) else {
                    return value;
                };
                let side = if matches!(segment, Segment::Pre(_)) {
                    &key.pre
                } else {
                    &key.post
                };
                for (index, (out, expression)) in value.iter_mut().zip(side).enumerate() {
                    *out = axis(expression, index);
                }
            }
            Segment::Linear { from, alpha } => {
                let (Some(a), Some(b)) = (keys.get(from), keys.get(from + 1)) else {
                    return value;
                };
                for (index, out) in value.iter_mut().enumerate() {
                    let (Some(p1), Some(p2)) = (a.post.get(index), b.pre.get(index)) else {
                        continue;
                    };
                    let p1 = axis(p1, index);
                    let p2 = axis(p2, index);
                    *out = p1 + (p2 - p1) * alpha;
                }
            }
            Segment::CatmullRom { from, alpha } => {
                let (Some(a), Some(b)) = (keys.get(from), keys.get(from + 1)) else {
                    return value;
                };
                let before = from.checked_sub(1).and_then(|index| keys.get(index));
                let after = keys.get(from + 2);
                for (index, out) in value.iter_mut().enumerate() {
                    let (Some(p1), Some(p2)) = (a.post.get(index), b.pre.get(index)) else {
                        continue;
                    };
                    let p1 = axis(p1, index);
                    let p2 = axis(p2, index);
                    let p0 = before
                        .and_then(|key| key.post.get(index))
                        .map_or(p1, |expression| axis(expression, index));
                    let p3 = after
                        .and_then(|key| key.pre.get(index))
                        .map_or(p2, |expression| axis(expression, index));
                    *out = catmull_rom([p0, p1, p2, p3], alpha);
                }
            }
        }
        value
    }

    /// Reads a channel of a bone: a value, or an object of keyframes. `None` for a content
    /// error.
    pub(crate) fn compile(
        json: &Json,
        compiler: &mut FieldCompiler,
        context: &str,
    ) -> Option<Self> {
        let Json::Object(object) = json else {
            let value = vector(json, compiler, context)?;
            return Some(Self {
                keyframes: vec![Keyframe {
                    time: 0.0,
                    pre: value.clone(),
                    post: value,
                    lerp_mode: LerpMode::Linear,
                }],
            });
        };
        let mut keyframes = Vec::with_capacity(object.len());
        for (key, value) in object.iter() {
            let context = format!("{context}/{key}");
            let time = match key.trim().parse::<f32>() {
                Ok(time) if time.is_finite() && time >= 0.0 => time,
                _ => {
                    compiler.diagnostics.push(AnimationDiagnostic::new(
                        DiagnosticKind::ContentError,
                        &context,
                        "a keyframe time is not a number of seconds",
                    ));
                    continue;
                }
            };
            if let Some(keyframe) = keyframe(time, value, compiler, &context) {
                keyframes.push(keyframe);
            }
        }
        keyframes.sort_by(|a, b| a.time.total_cmp(&b.time));
        Some(Self { keyframes })
    }

    /// The time of the last keyframe.
    #[must_use]
    pub fn end(&self) -> f32 {
        self.keyframes.last().map_or(0.0, |key| key.time)
    }
}

fn catmull_rom([p0, p1, p2, p3]: [f32; 4], alpha: f32) -> f32 {
    let alpha2 = alpha * alpha;
    let alpha3 = alpha2 * alpha;
    0.5 * (2.0 * p1
        + (p2 - p0) * alpha
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * alpha2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * alpha3)
}

fn keyframe(
    time: f32,
    json: &Json,
    compiler: &mut FieldCompiler,
    context: &str,
) -> Option<Keyframe> {
    let Json::Object(object) = json else {
        let value = vector(json, compiler, context)?;
        return Some(Keyframe {
            time,
            pre: value.clone(),
            post: value,
            lerp_mode: LerpMode::Linear,
        });
    };
    let pre = object
        .get("pre")
        .and_then(|value| vector(value, compiler, &format!("{context}/pre")));
    let post = object
        .get("post")
        .and_then(|value| vector(value, compiler, &format!("{context}/post")));
    let lerp_mode = match object.get("lerp_mode") {
        None => LerpMode::Linear,
        Some(Json::String(mode)) if mode == "linear" => LerpMode::Linear,
        Some(Json::String(mode)) if mode == "catmullrom" => LerpMode::CatmullRom,
        Some(_) => {
            compiler.diagnostics.push(AnimationDiagnostic::new(
                DiagnosticKind::ContentError,
                context,
                "`lerp_mode` is not `linear` or `catmullrom`",
            ));
            LerpMode::Linear
        }
    };
    let (pre, post) = match (pre, post) {
        (Some(pre), Some(post)) => (pre, post),
        (Some(value), None) | (None, Some(value)) => (value.clone(), value),
        (None, None) => {
            compiler.diagnostics.push(AnimationDiagnostic::new(
                DiagnosticKind::ContentError,
                context,
                "a keyframe object has no `pre` and no `post`",
            ));
            return None;
        }
    };
    Some(Keyframe {
        time,
        pre,
        post,
        lerp_mode,
    })
}

/// A value of a channel or a keyframe: a number or an expression for all three axes, or an
/// array of one or three of them.
fn vector(json: &Json, compiler: &mut FieldCompiler, context: &str) -> Option<[Expression; 3]> {
    let elements = match json {
        Json::Array(elements) => elements.as_slice(),
        single => core::slice::from_ref(single),
    };
    match elements {
        [one] if is_scalar(one) => {
            let expression = compiler.compile(one, context);
            Some([expression.clone(), expression.clone(), expression])
        }
        [x, y, z] if is_scalar(x) && is_scalar(y) && is_scalar(z) => Some([
            compiler.compile(x, &format!("{context}/x")),
            compiler.compile(y, &format!("{context}/y")),
            compiler.compile(z, &format!("{context}/z")),
        ]),
        _ => {
            compiler.diagnostics.push(AnimationDiagnostic::new(
                DiagnosticKind::ContentError,
                context,
                "a value is not a number, an expression or an array of one or three",
            ));
            None
        }
    }
}

fn is_scalar(json: &Json) -> bool {
    matches!(json, Json::Number(_) | Json::String(_) | Json::Bool(_))
}
