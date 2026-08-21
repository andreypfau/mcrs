use std::collections::{BTreeMap, HashMap};

use serde::de::IntoDeserializer;

use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::{Deserialize, Serialize};

use crate::attribute::{
    AttributeError, AttributeSpec, AttributeValue, Lerp, ModifierError, Operation, attribute,
};

#[derive(Debug, Clone, Deserialize, TypePath)]
pub struct Timeline {
    pub clock: String,
    #[serde(default)]
    pub period_ticks: Option<u32>,
    #[serde(default)]
    pub tracks: HashMap<String, Track>,
    #[serde(default)]
    pub time_markers: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub keyframes: Vec<Keyframe>,
    #[serde(default)]
    pub modifier: Option<String>,
    #[serde(default)]
    pub ease: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe {
    pub ticks: u32,
    pub value: serde_json::Value,
}

/// Timeline data subset for NETWORK_CODEC — mirrors the fields the vanilla
/// 26.1 client expects: clock, optional period_ticks, tracks, time_markers.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkTimeline {
    pub clock: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_ticks: Option<u32>,
    pub tracks: HashMap<String, Track>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub time_markers: HashMap<String, serde_json::Value>,
}

impl From<&Timeline> for NetworkTimeline {
    fn from(tl: &Timeline) -> Self {
        NetworkTimeline {
            clock: tl.clock.clone(),
            period_ticks: tl.period_ticks,
            tracks: tl.tracks.clone(),
            time_markers: tl.time_markers.clone(),
        }
    }
}

impl Asset for Timeline {}

impl VisitAssetDependencies for Timeline {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct TimelineLoader;

#[derive(Debug, thiserror::Error)]
pub enum TimelineLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AssetLoader for TimelineLoader {
    type Asset = Timeline;
    type Settings = ();
    type Error = TimelineLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Timeline, TimelineLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }
}

// ── Baking and sampling ──────────────────────────────────────────────────────

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
    /// An absent `ease` field means `linear`.
    pub fn parse(ease: Option<&serde_json::Value>) -> Result<Self, TrackError> {
        let Some(ease) = ease else {
            return Ok(Easing::Linear);
        };
        match ease {
            serde_json::Value::String(name) => match name.as_str() {
                "linear" => Ok(Easing::Linear),
                "constant" => Ok(Easing::Constant),
                _ => Err(TrackError::UnsupportedEasing(name.clone())),
            },
            serde_json::Value::Object(fields) if fields.len() == 1 => {
                let controls = fields
                    .get("cubic_bezier")
                    .and_then(serde_json::Value::as_array)
                    .filter(|controls| controls.len() == 4)
                    .map(|controls| {
                        controls
                            .iter()
                            .map(serde_json::Value::as_f64)
                            .collect::<Option<Vec<_>>>()
                    });
                match controls {
                    Some(Some(controls)) => CubicBezier::new(
                        controls[0] as f32,
                        controls[1] as f32,
                        controls[2] as f32,
                        controls[3] as f32,
                    )
                    .map(Easing::CubicBezier),
                    _ => match fields.keys().next().map(String::as_str) {
                        Some("cubic_bezier") => Err(TrackError::MalformedEasing(ease.clone())),
                        Some(name) => Err(TrackError::UnsupportedEasing(name.to_owned())),
                        None => Err(TrackError::MalformedEasing(ease.clone())),
                    },
                }
            }
            other => Err(TrackError::MalformedEasing(other.clone())),
        }
    }

    pub fn apply(self, x: f32) -> f32 {
        match self {
            Easing::Linear => x,
            Easing::Constant => 0.0,
            Easing::CubicBezier(bezier) => bezier.apply(x),
        }
    }
}

/// `EasingType.CubicBezier`, with its two cubics derived from the control
/// points once here rather than on every sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
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

#[derive(Debug, Clone)]
struct Segment {
    from_value: AttributeValue,
    /// Signed: the prepended wrap segment starts one period before the last
    /// keyframe, which is negative whenever the track ends before its period.
    from_ticks: i64,
    to_value: AttributeValue,
    to_ticks: i64,
}

/// `KeyframeTrackSampler`: a track's keyframes baked into contiguous segments.
///
/// The segment list is precomputation, not a cache: it is derived once from the
/// immutable keyframes of a loaded asset and there is no input that could
/// invalidate it. [`TrackSampler::sample`] is a pure function of the tick count
/// — the same ticks give the same value, and nothing is retained between calls.
#[derive(Debug, Clone)]
pub struct TrackSampler {
    period_ticks: Option<u32>,
    easing: Easing,
    lerp: Lerp,
    segments: Vec<Segment>,
}

impl TrackSampler {
    pub fn easing(&self) -> Easing {
        self.easing
    }

    pub fn sample(&self, ticks: i64) -> AttributeValue {
        let sample = match self.period_ticks {
            Some(period) => ticks.rem_euclid(i64::from(period)),
            None => ticks,
        };
        let segment = self
            .segments
            .iter()
            .find(|segment| sample < segment.to_ticks)
            .unwrap_or_else(|| self.segments.last().expect("a baked track has a segment"));

        if sample <= segment.from_ticks {
            return segment.from_value.clone();
        }
        if sample >= segment.to_ticks {
            return segment.to_value.clone();
        }
        // Both ends were just excluded, so `from_ticks < sample < to_ticks` and
        // the zero-length segment between two keyframes on the same tick — how
        // `sun_angle` writes a full revolution — never reaches the division.
        let alpha =
            (sample - segment.from_ticks) as f32 / (segment.to_ticks - segment.from_ticks) as f32;
        self.lerp.apply(
            self.easing.apply(alpha),
            &segment.from_value,
            &segment.to_value,
        )
    }
}

/// `AttributeTrackSampler`: a baked track together with the modifier whose
/// argument its keyframes hold.
///
/// Unlike the reference, which memoizes its last sampled argument against a
/// tick id and is poked from outside, this holds no sampled state at all.
#[derive(Debug, Clone)]
pub struct AttributeTrackSampler {
    pub attribute: &'static AttributeSpec,
    pub modifier: Operation,
    pub argument: TrackSampler,
}

impl AttributeTrackSampler {
    pub fn sample_argument(&self, ticks: i64) -> AttributeValue {
        self.argument.sample(ticks)
    }

    pub fn apply(
        &self,
        base: &AttributeValue,
        ticks: i64,
    ) -> Result<AttributeValue, ModifierError> {
        crate::attribute::apply(
            self.attribute.ty,
            self.modifier,
            base,
            &self.argument.sample(ticks),
        )
    }
}

#[derive(Debug, thiserror::Error)]
#[error("timeline track `{track}`: {kind}")]
pub struct TimelineError {
    pub track: String,
    #[source]
    pub kind: TrackError,
}

#[derive(Debug, thiserror::Error)]
pub enum TrackError {
    #[error("`{0}` is not an environment attribute; the registry is behind the game version")]
    UnknownAttribute(String),
    #[error("`{0}` is not a modifier operation")]
    UnknownModifier(String),
    #[error(
        "`{0}` is not a supported easing; only `linear`, `constant` and `cubic_bezier` are implemented"
    )]
    UnsupportedEasing(String),
    #[error("`ease` must be a name or `{{\"cubic_bezier\": [x1, y1, x2, y2]}}`, got {0}")]
    MalformedEasing(serde_json::Value),
    #[error("cubic_bezier control `{name}` is {value}, which is not in range [0; 1]")]
    BezierControl { name: &'static str, value: f32 },
    #[error("keyframes must not be empty")]
    NoKeyframes,
    #[error("keyframes must be ordered by ticks; {0} follows a later tick")]
    OutOfOrder(u32),
    #[error("more than 2 keyframes on tick {0}")]
    RepeatedTick(u32),
    #[error("keyframe at tick {ticks} must be in range [0; {period}]")]
    OutsidePeriod { ticks: u32, period: u32 },
    #[error(transparent)]
    Attribute(#[from] AttributeError),
}

impl Track {
    /// Validation happens here rather than at load: baking is the only
    /// consumer, so a track that never bakes can never be sampled wrong.
    pub fn bake(
        &self,
        attribute_id: &str,
        period_ticks: Option<u32>,
    ) -> Result<AttributeTrackSampler, TrackError> {
        let spec = attribute(attribute_id)
            .ok_or_else(|| TrackError::UnknownAttribute(attribute_id.to_owned()))?;
        let modifier = match &self.modifier {
            None => Operation::Override,
            Some(name) => Operation::deserialize(name.as_str().into_deserializer())
                .map_err(|_: serde::de::value::Error| TrackError::UnknownModifier(name.clone()))?,
        };
        let easing = Easing::parse(self.ease.as_ref())?;
        self.validate(period_ticks)?;

        let keyframes = self
            .keyframes
            .iter()
            .map(|keyframe| {
                Ok((
                    i64::from(keyframe.ticks),
                    spec.parse_argument(modifier, &keyframe.value)?,
                ))
            })
            .collect::<Result<Vec<_>, TrackError>>()?;

        Ok(AttributeTrackSampler {
            attribute: spec,
            modifier,
            argument: TrackSampler {
                period_ticks,
                easing,
                lerp: spec.argument_keyframe_lerp(modifier),
                segments: bake_segments(&keyframes, period_ticks),
            },
        })
    }

    /// `KeyframeTrack.validateKeyframes` and `validatePeriod`.
    fn validate(&self, period_ticks: Option<u32>) -> Result<(), TrackError> {
        let Some(first) = self.keyframes.first() else {
            return Err(TrackError::NoKeyframes);
        };
        let (mut previous, mut repeats) = (first.ticks, 0);
        for keyframe in &self.keyframes {
            if keyframe.ticks < previous {
                return Err(TrackError::OutOfOrder(keyframe.ticks));
            }
            repeats = if keyframe.ticks == previous {
                repeats + 1
            } else {
                1
            };
            if repeats > 2 {
                return Err(TrackError::RepeatedTick(keyframe.ticks));
            }
            if let Some(period) = period_ticks
                && keyframe.ticks > period
            {
                return Err(TrackError::OutsidePeriod {
                    ticks: keyframe.ticks,
                    period,
                });
            }
            previous = keyframe.ticks;
        }
        Ok(())
    }
}

/// `KeyframeTrackSampler.bakeSegments`.
///
/// A periodic track gets one wrap segment at each end so a sample either side
/// of the keyframe range interpolates around the period instead of clamping.
fn bake_segments(keyframes: &[(i64, AttributeValue)], period_ticks: Option<u32>) -> Vec<Segment> {
    let (first_ticks, first_value) = &keyframes[0];
    if keyframes.len() == 1 {
        return vec![Segment {
            from_value: first_value.clone(),
            from_ticks: 0,
            to_value: first_value.clone(),
            to_ticks: 0,
        }];
    }
    let (last_ticks, last_value) = keyframes.last().expect("more than one keyframe");

    let between = keyframes.windows(2).map(|pair| Segment {
        from_value: pair[0].1.clone(),
        from_ticks: pair[0].0,
        to_value: pair[1].1.clone(),
        to_ticks: pair[1].0,
    });
    let Some(period) = period_ticks.map(i64::from) else {
        return between.collect();
    };
    let wrap = |from_ticks, to_ticks| Segment {
        from_value: last_value.clone(),
        from_ticks,
        to_value: first_value.clone(),
        to_ticks,
    };
    std::iter::once(wrap(last_ticks - period, *first_ticks))
        .chain(between)
        .chain(std::iter::once(wrap(*last_ticks, first_ticks + period)))
        .collect()
}

impl Timeline {
    /// Bake every track of this timeline.
    ///
    /// Derived once from the loaded asset and never invalidated, so the result
    /// is worth keeping; the samplers it holds retain nothing themselves.
    pub fn bake(&self) -> Result<BTreeMap<String, AttributeTrackSampler>, TimelineError> {
        self.tracks
            .iter()
            .map(|(id, track)| {
                let sampler = track
                    .bake(id, self.period_ticks)
                    .map_err(|kind| TimelineError {
                        track: id.clone(),
                        kind,
                    })?;
                Ok((id.clone(), sampler))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn assets_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets")
    }

    #[test]
    fn network_timeline_round_trips_required_fields() {
        let bytes =
            std::fs::read(assets_dir().join("minecraft/timeline/villager_schedule.json")).unwrap();
        let timeline: Timeline = serde_json::from_slice(&bytes).unwrap();
        let network = NetworkTimeline::from(&timeline);
        let json = serde_json::to_value(&network).unwrap();
        assert!(json.get("clock").is_some());
        assert!(json.get("tracks").is_some());
        assert_eq!(
            json.get("period_ticks").and_then(|v| v.as_u64()),
            Some(24000)
        );
    }

    #[test]
    fn deserialize_all_timelines() {
        let dir = assets_dir().join("minecraft/timeline");
        let mut count = 0;
        let mut failures = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("timeline dir must exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            match serde_json::from_slice::<Timeline>(&bytes) {
                Ok(_) => count += 1,
                Err(e) => failures.push((path.display().to_string(), e.to_string())),
            }
        }

        if !failures.is_empty() {
            for (path, err) in &failures {
                eprintln!("FAIL {path}: {err}");
            }
            panic!(
                "{} of {} timelines failed to deserialize",
                failures.len(),
                count + failures.len()
            );
        }

        assert!(count > 0, "no timeline files found");
        eprintln!("successfully deserialized {count} timelines");
    }

    // ── Baking and sampling ──────────────────────────────────────────────────

    const DAY: f32 = 24000.0;

    fn timeline(name: &str) -> Timeline {
        let bytes = std::fs::read(assets_dir().join("minecraft/timeline").join(name)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn float(value: &AttributeValue) -> f32 {
        match value {
            AttributeValue::Float(v) => *v,
            other => panic!("expected a float, got {other:?}"),
        }
    }

    fn channels(color: u32) -> [f32; 4] {
        [24, 16, 8, 0].map(|shift| (color >> shift & 0xFF) as f32)
    }

    /// Whatever an argument parses to, as four channels: a colour's bytes, or a
    /// float in the first slot. Lets one oracle cover both kinds of track.
    fn as_channels(value: &AttributeValue) -> [f32; 4] {
        match value {
            AttributeValue::Float(v) => [*v, 0.0, 0.0, 0.0],
            AttributeValue::Color(c) => channels(*c),
            other => panic!("expected a float or a colour, got {other:?}"),
        }
    }

    /// The keyframes of one track, read back through the registry the same way
    /// the sampler reads them.
    fn keys(timeline: &Timeline, id: &str) -> Vec<(f32, [f32; 4])> {
        let track = &timeline.tracks[id];
        let sampler = track.bake(id, timeline.period_ticks).unwrap();
        track
            .keyframes
            .iter()
            .map(|keyframe| {
                let value = sampler
                    .attribute
                    .parse_argument(sampler.modifier, &keyframe.value)
                    .unwrap();
                (keyframe.ticks as f32, as_channels(&value))
            })
            .collect()
    }

    /// The wrap-and-lerp of `examples/anvil_region_viewer/daylight.rs::track`,
    /// in float space, as an oracle independent of the baked segment list.
    fn daylight_track(keys: &[(f32, [f32; 4])], ticks: f32) -> [f32; 4] {
        let mut at = ticks.rem_euclid(DAY);
        if at < keys[0].0 {
            at += DAY;
        }
        for (index, &(start, value)) in keys.iter().enumerate() {
            let (mut end, next) = keys[(index + 1) % keys.len()];
            if index + 1 == keys.len() {
                end += DAY;
            }
            if at <= end {
                let fraction = (at - start) / (end - start);
                let mut blended = [0.0; 4];
                for channel in 0..4 {
                    blended[channel] = value[channel] * (1.0 - fraction) + next[channel] * fraction;
                }
                return blended;
            }
        }
        keys[0].1
    }

    /// `daylight.rs::sun_angle`, in degrees: vanilla's old closed-form
    /// approximation of the cubic bezier the track now names.
    fn daylight_sun_angle(ticks: f32) -> f32 {
        let day = (ticks / DAY - 0.25).rem_euclid(1.0);
        let eased = 0.5 - (day * std::f32::consts::PI).cos() * 0.5;
        (day * 2.0 + eased) / 3.0 * 360.0
    }

    #[test]
    fn every_shipped_track_bakes_and_samples() {
        let expected = [
            ("day.json", 18, Some(24000)),
            ("moon.json", 2, Some(192000)),
            ("villager_schedule.json", 2, Some(24000)),
            ("early_game.json", 1, None),
        ];
        for (name, tracks, period) in expected {
            let timeline = timeline(name);
            assert_eq!(timeline.period_ticks, period, "{name} period");
            let baked = timeline.bake().unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(baked.len(), tracks, "{name} track count");

            for (id, sampler) in &baked {
                for ticks in (-50000..250000).step_by(997) {
                    let sampled = sampler.sample_argument(ticks);
                    assert_eq!(
                        sampled,
                        sampler.sample_argument(ticks),
                        "{name} / {id} at {ticks} is not a pure function of ticks"
                    );
                }
            }
        }
    }

    #[test]
    fn only_the_easings_the_assets_use_are_implemented() {
        let track = |ease: serde_json::Value| Track {
            keyframes: vec![
                Keyframe {
                    ticks: 0,
                    value: serde_json::json!(0.0),
                },
                Keyframe {
                    ticks: 100,
                    value: serde_json::json!(1.0),
                },
            ],
            modifier: Some("multiply".to_owned()),
            ease: Some(ease),
        };
        let bake = |ease: serde_json::Value| {
            track(ease).bake("minecraft:visual/sky_light_factor", Some(24000))
        };

        assert_eq!(
            bake(serde_json::json!("linear")).unwrap().argument.easing(),
            Easing::Linear
        );
        assert_eq!(
            bake(serde_json::json!("constant"))
                .unwrap()
                .argument
                .easing(),
            Easing::Constant
        );
        assert!(matches!(
            bake(serde_json::json!({"cubic_bezier": [0.362, 0.241, 0.638, 0.759]}))
                .unwrap()
                .argument
                .easing(),
            Easing::CubicBezier(_)
        ));

        // a curve the reference registers but we do not implement must not
        // quietly behave as linear
        let err = bake(serde_json::json!("in_out_bounce"))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("in_out_bounce") && err.contains("not a supported easing"),
            "{err}"
        );
        assert!(bake(serde_json::json!("nonsense")).is_err());
        assert!(
            bake(serde_json::json!({"cubic_bezier": [0.5, 0.5, 0.5]})).is_err(),
            "needs four"
        );
        assert!(
            bake(serde_json::json!({"cubic_bezier": [1.5, 0.0, 0.5, 1.0]})).is_err(),
            "x1 must be in [0; 1]"
        );
    }

    #[test]
    fn the_daylight_tables_are_the_day_json_tracks() {
        // hand-extracted in examples/anvil_region_viewer/daylight.rs
        const SKY_LIGHT_FACTOR: [(f32, f32); 4] = [
            (730.0, 1.0),
            (11270.0, 1.0),
            (13140.0, 0.24),
            (22860.0, 0.24),
        ];
        const STAR_BRIGHTNESS: [(f32, f32); 12] = [
            (92.0, 0.037),
            (627.0, 0.0),
            (11373.0, 0.0),
            (11732.0, 0.016),
            (11959.0, 0.044),
            (12399.0, 0.143),
            (12729.0, 0.258),
            (13228.0, 0.5),
            (22772.0, 0.5),
            (23032.0, 0.364),
            (23356.0, 0.225),
            (23758.0, 0.101),
        ];
        // the multiply factor daylight.rs writes as a scalar where day.json
        // writes #ffffff and #000000
        const SKY_COLOR: [(f32, f32); 4] =
            [(133.0, 1.0), (11867.0, 1.0), (13670.0, 0.0), (22330.0, 0.0)];

        let day = timeline("day.json");
        for (id, table) in [
            ("minecraft:visual/sky_light_factor", &SKY_LIGHT_FACTOR[..]),
            ("minecraft:visual/star_brightness", &STAR_BRIGHTNESS[..]),
        ] {
            let baked: Vec<_> = keys(&day, id).iter().map(|&(t, v)| (t, v[0])).collect();
            assert_eq!(baked, table, "{id} does not match its daylight.rs table");
        }

        let sky_color: Vec<_> = keys(&day, "minecraft:visual/sky_color")
            .iter()
            .map(|&(t, v)| (t, v[1] / 255.0))
            .collect();
        assert_eq!(sky_color, SKY_COLOR);
    }

    #[test]
    fn sampling_matches_the_daylight_tables_at_and_between_their_keys() {
        let day = timeline("day.json");
        let interpolated = [
            "minecraft:gameplay/sky_light_level",
            "minecraft:visual/cloud_color",
            "minecraft:visual/fog_color",
            "minecraft:visual/sky_color",
            "minecraft:visual/sky_light_color",
            "minecraft:visual/sky_light_factor",
            "minecraft:visual/star_brightness",
            "minecraft:visual/sunrise_sunset_color",
        ];

        for id in interpolated {
            let table = keys(&day, id);
            let sampler = day.tracks[id].bake(id, day.period_ticks).unwrap();
            let is_color = matches!(sampler.sample_argument(0), AttributeValue::Color(_));

            let ticks = table
                .iter()
                .map(|&(tick, _)| tick as i64)
                .chain((-24000..48000).step_by(37))
                .collect::<Vec<_>>();

            for tick in ticks {
                let sampled = as_channels(&sampler.sample_argument(tick));
                let expected = daylight_track(&table, tick as f32);
                for channel in 0..if is_color { 4 } else { 1 } {
                    let (got, want) = (sampled[channel], expected[channel]);
                    // an integer channel lerp floors, so a byte channel may sit
                    // one below the float oracle
                    let tolerance = if is_color { 1.0 } else { 1e-5 };
                    assert!(
                        (got - want).abs() <= tolerance,
                        "{id} channel {channel} at {tick}: got {got}, daylight.rs says {want}"
                    );
                }
            }
        }
    }

    #[test]
    fn sun_angle_agrees_with_the_daylight_closed_form_across_a_day() {
        let day = timeline("day.json");
        let sun = day.tracks["minecraft:visual/sun_angle"]
            .bake("minecraft:visual/sun_angle", day.period_ticks)
            .unwrap();
        let moon = day.tracks["minecraft:visual/moon_angle"]
            .bake("minecraft:visual/moon_angle", day.period_ticks)
            .unwrap();

        let mut worst: f32 = 0.0;
        for tick in 0..24000 {
            let sampled = float(&sun.sample_argument(tick));
            worst = worst.max((sampled - daylight_sun_angle(tick as f32)).abs());
            assert_eq!(
                float(&moon.sample_argument(tick)),
                sampled + 180.0,
                "the moon trails the sun by half a revolution at {tick}"
            );
        }
        assert!(
            worst < 0.1,
            "the bezier drifts {worst} degrees from the closed form"
        );

        // the anchors daylight.rs pins: noon starts the revolution, dusk is a
        // quarter of the way round, midnight is halfway
        assert_eq!(float(&sun.sample_argument(6000)), 0.0);
        assert!((float(&sun.sample_argument(18000)) - 180.0).abs() < 0.1);
        assert!(float(&sun.sample_argument(12000)) > 60.0);
        assert!(float(&sun.sample_argument(12000)) < 90.0);
    }

    #[test]
    fn two_keyframes_on_one_tick_make_a_full_revolution() {
        let day = timeline("day.json");
        let sun = day.tracks["minecraft:visual/sun_angle"]
            .bake("minecraft:visual/sun_angle", day.period_ticks)
            .unwrap();

        // the zero-length middle segment is what makes tick 6000 read as 0
        assert_eq!(float(&sun.sample_argument(6000)), 0.0);
        assert!(
            float(&sun.sample_argument(5999)) > 359.9,
            "and the tick before it as 360"
        );
        assert!(float(&sun.sample_argument(6001)) < 0.1);
        let mut previous = 0.0;
        for tick in 6001..30000 {
            let angle = float(&sun.sample_argument(tick));
            assert!(angle >= previous, "sun_angle went backwards at {tick}");
            previous = angle;
        }
    }

    #[test]
    fn a_periodic_track_wraps_instead_of_clamping() {
        let day = timeline("day.json");
        let id = "minecraft:visual/sky_light_factor";
        let sampler = day.tracks[id].bake(id, day.period_ticks).unwrap();
        let at = |tick| float(&sampler.sample_argument(tick));

        // the keyframes run 730 → 22860, so 100 is before the first and 23000
        // after the last; both must interpolate around the period
        let before = at(100);
        let after = at(23000);
        assert!(
            before > 0.24 && before < 1.0,
            "before the first keyframe: {before}"
        );
        assert!(
            after > 0.24 && after < 1.0,
            "after the last keyframe: {after}"
        );
        assert!(after < before, "dawn climbs back towards full daylight");
        assert_eq!(
            at(-1000),
            at(23000),
            "negative ticks floor-mod into the period"
        );
        assert_eq!(at(24100), before);

        // the plateaus daylight.rs pins
        assert_eq!(at(6000), 1.0);
        assert_eq!(at(18000), 0.24);
    }

    #[test]
    fn a_track_without_a_period_neither_wraps_nor_reduces_its_ticks() {
        let early = timeline("early_game.json");
        assert_eq!(early.period_ticks, None);
        let id = "minecraft:gameplay/can_pillager_patrol_spawn";
        let sampler = early.bake().unwrap().remove(id).unwrap();
        let at = |tick| sampler.sample_argument(tick);

        let (no, yes) = (AttributeValue::Bool(false), AttributeValue::Bool(true));
        assert_eq!(at(-50_000), no, "before the first keyframe the track holds");
        assert_eq!(at(0), no);
        // boolean arguments step, so the switch happens at the far keyframe
        assert_eq!(at(119_999), no);
        assert_eq!(at(120_000), yes);
        assert_eq!(at(1_000_000), yes, "and never wraps back round");

        assert_eq!(sampler.modifier, Operation::And);
        assert_eq!(sampler.apply(&yes, 0).unwrap(), no);
        assert_eq!(sampler.apply(&yes, 120_000).unwrap(), yes);
    }

    #[test]
    fn a_not_interpolated_track_holds_each_value_for_a_whole_segment() {
        let moon = timeline("moon.json");
        let id = "minecraft:visual/moon_phase";
        let sampler = moon.tracks[id].bake(id, moon.period_ticks).unwrap();
        let at = |tick| sampler.sample_argument(tick);
        let phase = |name: &str| AttributeValue::Opaque(serde_json::json!(name));

        // nominally a linear track, but MOON_PHASE is not interpolated, so each
        // phase holds for a whole day instead of blending into the next
        assert_eq!(at(0), phase("full_moon"));
        assert_eq!(at(12_000), phase("full_moon"));
        assert_eq!(at(23_999), phase("full_moon"));
        assert_eq!(at(24_000), phase("waning_gibbous"));
        assert_eq!(
            at(192_000),
            phase("full_moon"),
            "the period brings it back round"
        );
        assert_eq!(
            at(-1),
            phase("waxing_gibbous"),
            "and the tick before it is the last phase"
        );
    }

    #[test]
    fn a_constant_easing_holds_until_the_next_keyframe() {
        let day = timeline("day.json");
        let id = "minecraft:gameplay/cat_waking_up_gift_chance";
        let sampler = day.tracks[id].bake(id, day.period_ticks).unwrap();
        assert_eq!(sampler.argument.easing(), Easing::Constant);

        // keyframes are 362 → 0.0 and 23667 → 0.7
        assert_eq!(float(&sampler.sample_argument(362)), 0.0);
        assert_eq!(float(&sampler.sample_argument(12_000)), 0.0);
        assert_eq!(float(&sampler.sample_argument(23_666)), 0.0);
        assert_eq!(float(&sampler.sample_argument(23_667)), 0.7);
    }

    #[test]
    fn malformed_tracks_are_rejected() {
        let track = |keyframes: Vec<(u32, f32)>| Track {
            keyframes: keyframes
                .into_iter()
                .map(|(ticks, value)| Keyframe {
                    ticks,
                    value: serde_json::json!(value),
                })
                .collect(),
            modifier: Some("multiply".to_owned()),
            ease: None,
        };
        let bake =
            |keyframes, period| track(keyframes).bake("minecraft:visual/sky_light_factor", period);

        assert!(
            bake(vec![], Some(24000)).is_err(),
            "keyframes must not be empty"
        );
        assert!(
            bake(vec![(100, 1.0), (50, 0.0)], Some(24000)).is_err(),
            "must be ordered"
        );
        assert!(
            bake(vec![(50, 1.0), (50, 0.5), (50, 0.0)], Some(24000)).is_err(),
            "at most two keyframes may share a tick"
        );
        assert!(
            bake(vec![(50, 1.0), (50, 0.0)], Some(24000)).is_ok(),
            "but two may"
        );
        assert!(
            bake(vec![(0, 1.0), (24001, 0.0)], Some(24000)).is_err(),
            "must be within period"
        );
        assert!(
            bake(vec![(0, 1.0), (24001, 0.0)], None).is_ok(),
            "unless there is no period"
        );

        let unknown = Track {
            keyframes: vec![],
            modifier: None,
            ease: None,
        }
        .bake("minecraft:visual/sky_colour", None)
        .unwrap_err()
        .to_string();
        assert!(
            unknown.contains("is not an environment attribute"),
            "{unknown}"
        );

        let bad_modifier = Track {
            keyframes: vec![Keyframe {
                ticks: 0,
                value: serde_json::json!(1.0),
            }],
            modifier: Some("blend".to_owned()),
            ease: None,
        }
        .bake("minecraft:visual/sky_light_factor", None)
        .unwrap_err()
        .to_string();
        assert!(
            bad_modifier.contains("not a modifier operation"),
            "{bad_modifier}"
        );
    }

    #[test]
    fn a_single_keyframe_track_is_a_constant() {
        let track = Track {
            keyframes: vec![Keyframe {
                ticks: 500,
                value: serde_json::json!(0.25),
            }],
            modifier: Some("multiply".to_owned()),
            ease: None,
        };
        for period in [Some(24000), None] {
            let sampler = track
                .bake("minecraft:visual/sky_light_factor", period)
                .unwrap();
            for tick in [-1_000_000, -1, 0, 500, 23_999, 1_000_000] {
                assert_eq!(float(&sampler.sample_argument(tick)), 0.25, "at {tick}");
            }
        }
    }

    #[test]
    fn baking_leaves_the_network_json_untouched() {
        let raw: serde_json::Value = serde_json::from_slice(
            &std::fs::read(assets_dir().join("minecraft/timeline/day.json")).unwrap(),
        )
        .unwrap();
        let timeline: Timeline = serde_json::from_value(raw.clone()).unwrap();
        timeline.bake().unwrap();

        let network = serde_json::to_value(NetworkTimeline::from(&timeline)).unwrap();
        assert_eq!(network["clock"], raw["clock"]);
        assert_eq!(network["period_ticks"], raw["period_ticks"]);
        assert_eq!(network["time_markers"], raw["time_markers"]);
        for (id, track) in raw["tracks"].as_object().unwrap() {
            // the typed keyframes and the parsed easing are derived, never
            // written back: what goes out to the client is the input verbatim
            assert_eq!(
                network["tracks"][id]["keyframes"], track["keyframes"],
                "{id}"
            );
            assert_eq!(
                network["tracks"][id]["ease"],
                track
                    .get("ease")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
                "{id}"
            );
        }
    }
}
