use std::collections::{BTreeMap, HashMap};
use std::ops::Deref;
use std::sync::Arc;

use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetLoader, LoadContext, UntypedAssetId, VisitAssetDependencies};
use bevy_reflect::TypePath;
use serde::de::{DeserializeSeed, Error as _, MapAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use mcrs_core::tag::key::TaggedRegistry;

use crate::ResourceLocation;
use crate::attribute::{
    AttributeSpec, AttributeValue, Lerp, ModifierError, Operation, RawArgument, attribute,
};

#[derive(Debug, Clone, TypePath)]
pub struct Timeline {
    pub clock: ResourceLocation<Arc<str>>,
    pub period_ticks: Option<u32>,
    pub tracks: Tracks,
    pub time_markers: HashMap<String, TimeMarker>,
}

impl TaggedRegistry for Timeline {
    const REGISTRY_PATH: &'static str = "timeline";
}

#[derive(Deserialize)]
struct TimelineRepr {
    clock: ResourceLocation<Arc<str>>,
    #[serde(default)]
    period_ticks: Option<u32>,
    #[serde(default)]
    tracks: Tracks,
    #[serde(default)]
    time_markers: HashMap<String, TimeMarker>,
}

impl<'de> Deserialize<'de> for Timeline {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let repr = TimelineRepr::deserialize(d)?;
        if let Some(period) = repr.period_ticks {
            for (id, track) in repr.tracks.iter() {
                track.validate_period(period).map_err(|kind| {
                    D::Error::custom(TimelineError {
                        track: (*id).to_owned(),
                        kind,
                    })
                })?;
            }
        }
        Ok(Timeline {
            clock: repr.clock,
            period_ticks: repr.period_ticks,
            tracks: repr.tracks,
            time_markers: repr.time_markers,
        })
    }
}

/// A time marker as a timeline declares it: a bare tick count, or an object
/// that also asks for the marker to be offered to commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeMarker {
    pub ticks: u32,
    pub show_in_commands: bool,
}

#[derive(Serialize, Deserialize)]
struct FullTimeMarker {
    ticks: u32,
    #[serde(default)]
    show_in_commands: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TimeMarkerRepr {
    Bare(u32),
    Full(FullTimeMarker),
}

impl<'de> Deserialize<'de> for TimeMarker {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match TimeMarkerRepr::deserialize(d)? {
            TimeMarkerRepr::Bare(ticks) => TimeMarker {
                ticks,
                show_in_commands: false,
            },
            TimeMarkerRepr::Full(full) => TimeMarker {
                ticks: full.ticks,
                show_in_commands: full.show_in_commands,
            },
        })
    }
}

impl Serialize for TimeMarker {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.show_in_commands {
            FullTimeMarker {
                ticks: self.ticks,
                show_in_commands: true,
            }
            .serialize(s)
        } else {
            s.serialize_u32(self.ticks)
        }
    }
}

/// `Timeline.TRACKS_CODEC`, a `Codec.dispatchedMap`: the attribute a key names
/// chooses the type its track's keyframe values are read and written in.
#[derive(Debug, Clone, Default)]
pub struct Tracks(BTreeMap<&'static str, Track>);

impl Deref for Tracks {
    type Target = BTreeMap<&'static str, Track>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromIterator<Track> for Tracks {
    fn from_iter<I: IntoIterator<Item = Track>>(tracks: I) -> Self {
        Tracks(
            tracks
                .into_iter()
                .map(|track| (track.attribute.id, track))
                .collect(),
        )
    }
}

impl<'de> Deserialize<'de> for Tracks {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_map(TracksVisitor)
    }
}

struct TracksVisitor;

impl<'de> Visitor<'de> for TracksVisitor {
    type Value = Tracks;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a map of environment attribute id to track")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Tracks, A::Error> {
        let mut tracks = BTreeMap::new();
        while let Some(id) = map.next_key::<String>()? {
            let spec = attribute(&id)
                .ok_or_else(|| A::Error::custom(TrackError::UnknownAttribute(id.clone())))?;
            let track = map
                .next_value_seed(TrackSeed(spec))
                .map_err(|e| A::Error::custom(format!("timeline track `{id}`: {e}")))?;
            tracks.insert(spec.id, track);
        }
        Ok(Tracks(tracks))
    }
}

impl Serialize for Tracks {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (id, track) in &self.0 {
            map.serialize_entry(id, track)?;
        }
        map.end()
    }
}

#[derive(Debug, Clone)]
pub struct Track {
    pub attribute: &'static AttributeSpec,
    pub keyframes: Vec<Keyframe>,
    pub modifier: Option<Operation>,
    pub ease: Option<Easing>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe {
    pub ticks: u32,
    pub value: AttributeValue,
}

/// `AttributeTrack.createCodec(attribute)`: the seed the map key hands its
/// value, so a keyframe is parsed in the type the (attribute, modifier) pair
/// selects rather than in whatever shape the JSON happened to have.
struct TrackSeed(&'static AttributeSpec);

impl<'de> DeserializeSeed<'de> for TrackSeed {
    type Value = Track;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Track, D::Error> {
        #[derive(Deserialize)]
        struct RawKeyframe {
            ticks: u32,
            value: RawArgument,
        }

        #[derive(Deserialize)]
        struct RawTrack {
            keyframes: Vec<RawKeyframe>,
            #[serde(default)]
            modifier: Option<Operation>,
            #[serde(default)]
            ease: Option<Easing>,
        }

        let raw = RawTrack::deserialize(d)?;
        let modifier = raw.modifier.unwrap_or(Operation::Override);
        let keyframes = raw
            .keyframes
            .iter()
            .map(|keyframe| {
                Ok(Keyframe {
                    ticks: keyframe.ticks,
                    value: keyframe
                        .value
                        .parse(self.0, modifier)
                        .map_err(D::Error::custom)?,
                })
            })
            .collect::<Result<Vec<_>, D::Error>>()?;
        validate_keyframes(&keyframes).map_err(D::Error::custom)?;

        Ok(Track {
            attribute: self.0,
            keyframes,
            modifier: raw.modifier,
            ease: raw.ease,
        })
    }
}

impl Serialize for Track {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(
            1 + usize::from(self.modifier.is_some()) + usize::from(self.ease.is_some()),
        ))?;
        map.serialize_entry("keyframes", &Keyframes(self))?;
        if let Some(modifier) = &self.modifier {
            map.serialize_entry("modifier", modifier)?;
        }
        if let Some(ease) = &self.ease {
            map.serialize_entry("ease", ease)?;
        }
        map.end()
    }
}

struct Keyframes<'a>(&'a Track);

impl Serialize for Keyframes<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.0.keyframes.len()))?;
        for keyframe in &self.0.keyframes {
            seq.serialize_element(&KeyframeEntry {
                track: self.0,
                keyframe,
            })?;
        }
        seq.end()
    }
}

struct KeyframeEntry<'a> {
    track: &'a Track,
    keyframe: &'a Keyframe,
}

impl Serialize for KeyframeEntry<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(2))?;
        map.serialize_entry("ticks", &self.keyframe.ticks)?;
        map.serialize_entry(
            "value",
            &Argument {
                spec: self.track.attribute,
                op: self.track.operation(),
                value: &self.keyframe.value,
            },
        )?;
        map.end()
    }
}

struct Argument<'a> {
    spec: &'static AttributeSpec,
    op: Operation,
    value: &'a AttributeValue,
}

impl Serialize for Argument<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.spec.serialize_argument(self.op, self.value, s)
    }
}

/// Timeline data subset for NETWORK_CODEC — mirrors the fields the vanilla
/// 26.1 client expects: clock, optional period_ticks, tracks, time_markers.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkTimeline {
    pub clock: ResourceLocation<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_ticks: Option<u32>,
    pub tracks: Tracks,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub time_markers: HashMap<String, TimeMarker>,
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
        self.sample_at(ticks as f64)
    }

    /// Sample between two ticks.
    ///
    /// A track is already continuous across its own period, so a fractional
    /// tick needs no history: there is nothing to blend the result against.
    pub fn sample_at(&self, ticks: f64) -> AttributeValue {
        let sample = match self.period_ticks {
            Some(period) => ticks.rem_euclid(f64::from(period)),
            None => ticks,
        };
        let segment = self
            .segments
            .iter()
            .find(|segment| sample < segment.to_ticks as f64)
            .unwrap_or_else(|| self.segments.last().expect("a baked track has a segment"));

        if sample <= segment.from_ticks as f64 {
            return segment.from_value.clone();
        }
        if sample >= segment.to_ticks as f64 {
            return segment.to_value.clone();
        }
        // Both ends were just excluded, so `from_ticks < sample < to_ticks` and
        // the zero-length segment between two keyframes on the same tick — how
        // `sun_angle` writes a full revolution — never reaches the division.
        let alpha = ((sample - segment.from_ticks as f64)
            / (segment.to_ticks - segment.from_ticks) as f64) as f32;
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
        self.apply_at(base, ticks as f64)
    }

    pub fn apply_at(
        &self,
        base: &AttributeValue,
        ticks: f64,
    ) -> Result<AttributeValue, ModifierError> {
        crate::attribute::apply(
            self.attribute.ty,
            self.modifier,
            base,
            &self.argument.sample_at(ticks),
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
    #[error(
        "`{0}` is not a supported easing; only `linear`, `constant` and `cubic_bezier` are implemented"
    )]
    UnsupportedEasing(String),
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
}

impl Track {
    /// An absent `modifier` field means `override`.
    pub fn operation(&self) -> Operation {
        self.modifier.unwrap_or(Operation::Override)
    }

    /// Everything a sampler needs, derived from keyframes that were already
    /// typed and ordered when the asset loaded.
    pub fn bake(&self, period_ticks: Option<u32>) -> AttributeTrackSampler {
        let modifier = self.operation();
        let keyframes: Vec<(i64, AttributeValue)> = self
            .keyframes
            .iter()
            .map(|keyframe| (i64::from(keyframe.ticks), keyframe.value.clone()))
            .collect();

        AttributeTrackSampler {
            attribute: self.attribute,
            modifier,
            argument: TrackSampler {
                period_ticks,
                easing: self.ease.unwrap_or(Easing::Linear),
                lerp: self.attribute.argument_keyframe_lerp(modifier),
                segments: bake_segments(&keyframes, period_ticks),
            },
        }
    }

    /// `KeyframeTrack.validatePeriod`. The period is a sibling of `tracks`, so
    /// this is the one check the track's own seed cannot make.
    fn validate_period(&self, period: u32) -> Result<(), TrackError> {
        match self.keyframes.iter().find(|k| k.ticks > period) {
            Some(keyframe) => Err(TrackError::OutsidePeriod {
                ticks: keyframe.ticks,
                period,
            }),
            None => Ok(()),
        }
    }
}

/// `KeyframeTrack.validateKeyframes`.
fn validate_keyframes(keyframes: &[Keyframe]) -> Result<(), TrackError> {
    let Some(first) = keyframes.first() else {
        return Err(TrackError::NoKeyframes);
    };
    let (mut previous, mut repeats) = (first.ticks, 0);
    for keyframe in keyframes {
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
        previous = keyframe.ticks;
    }
    Ok(())
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
    /// The tick this timeline stands at within its own period.
    pub fn current_ticks(&self, total_ticks: i64) -> i64 {
        match self.period_ticks {
            Some(period) => total_ticks.rem_euclid(i64::from(period)),
            None => total_ticks,
        }
    }

    /// How many whole periods this timeline has completed.
    pub fn period_count(&self, total_ticks: i64) -> i64 {
        match self.period_ticks {
            Some(period) => total_ticks.div_euclid(i64::from(period)),
            None => 0,
        }
    }

    /// Bake every track of this timeline.
    ///
    /// Derived once from the loaded asset and never invalidated, so the result
    /// is worth keeping; the samplers it holds retain nothing themselves.
    pub fn bake(&self) -> BTreeMap<&'static str, AttributeTrackSampler> {
        self.tracks
            .iter()
            .map(|(id, track)| (*id, track.bake(self.period_ticks)))
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_nbt::tag::NbtTag;
    use serde_json::{Value, json};
    use std::path::PathBuf;

    const SHIPPED: [&str; 4] = [
        "day.json",
        "moon.json",
        "villager_schedule.json",
        "early_game.json",
    ];

    fn assets_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets")
    }

    fn raw(name: &str) -> Value {
        let bytes = std::fs::read(assets_dir().join("minecraft/timeline").join(name)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn timeline(name: &str) -> Timeline {
        serde_json::from_value(raw(name)).unwrap()
    }

    /// A one-track timeline read the way a datapack reaches one.
    fn load(attribute: &str, period_ticks: Option<u32>, track: Value) -> Result<Timeline, String> {
        let mut doc = json!({"clock": "minecraft:overworld", "tracks": {attribute: track}});
        if let Some(period) = period_ticks {
            doc["period_ticks"] = json!(period);
        }
        serde_json::from_value(doc).map_err(|e| e.to_string())
    }

    fn bake_one(
        attribute: &str,
        period_ticks: Option<u32>,
        track: Value,
    ) -> Result<AttributeTrackSampler, String> {
        let timeline = load(attribute, period_ticks, track)?;
        Ok(timeline.tracks[attribute].bake(timeline.period_ticks))
    }

    /// What the client actually receives, read back as JSON.
    ///
    /// Through the text, never `to_value`: a keyframe holds an `f32` and
    /// `serde_json::Value` has only `f64`, so `to_value` would widen `0.362`
    /// into `0.3619999885559082` where the encoder writes `0.362`.
    fn sent(timeline: &Timeline) -> Value {
        let text = serde_json::to_string(&NetworkTimeline::from(timeline)).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn network_timeline_round_trips_required_fields() {
        let json = sent(&timeline("villager_schedule.json"));
        assert!(json.get("clock").is_some());
        assert!(json.get("tracks").is_some());
        assert_eq!(
            json.get("period_ticks").and_then(|v| v.as_u64()),
            Some(24000)
        );
    }

    #[test]
    fn every_shipped_timeline_survives_the_network_round_trip_unchanged() {
        for name in SHIPPED {
            assert_eq!(sent(&timeline(name)), raw(name), "{name}");
        }
    }

    #[test]
    fn both_easing_shapes_and_both_marker_shapes_survive_the_round_trip() {
        let day = timeline("day.json");

        assert_eq!(
            day.tracks["minecraft:visual/sun_angle"].ease,
            Some(Easing::CubicBezier(
                CubicBezier::new(0.362, 0.241, 0.638, 0.759).unwrap()
            ))
        );
        assert_eq!(
            day.tracks["minecraft:gameplay/cat_waking_up_gift_chance"].ease,
            Some(Easing::Constant)
        );
        assert_eq!(day.tracks["minecraft:visual/sky_color"].ease, None);
        assert_eq!(
            day.time_markers["minecraft:day"],
            TimeMarker {
                ticks: 1000,
                show_in_commands: true
            }
        );
        assert_eq!(
            day.time_markers["minecraft:roll_village_siege"],
            TimeMarker {
                ticks: 18000,
                show_in_commands: false
            }
        );

        let sent = sent(&day);
        // the object easing keeps its four controls, the named one stays a name
        assert_eq!(
            sent["tracks"]["minecraft:visual/sun_angle"]["ease"],
            json!({"cubic_bezier": [0.362, 0.241, 0.638, 0.759]})
        );
        assert_eq!(
            sent["tracks"]["minecraft:gameplay/cat_waking_up_gift_chance"]["ease"],
            json!("constant")
        );
        assert!(
            sent["tracks"]["minecraft:visual/sky_color"]
                .get("ease")
                .is_none()
        );
        // an absent modifier stays absent rather than being written as `override`
        assert!(
            sent["tracks"]["minecraft:visual/sun_angle"]
                .get("modifier")
                .is_none()
        );

        assert_eq!(sent["time_markers"], raw("day.json")["time_markers"]);
        assert!(sent["time_markers"]["minecraft:roll_village_siege"].is_number());
        assert!(sent["time_markers"]["minecraft:day"].is_object());
    }

    #[test]
    fn the_nbt_form_types_every_keyframe_the_way_the_client_reads_it() {
        let day = timeline("day.json");
        let nbt = mcrs_minecraft_nbt::to_nbt_compound(&NetworkTimeline::from(&day)).unwrap();

        assert_eq!(nbt.get_string("clock"), Some("minecraft:overworld"));
        assert_eq!(nbt.get("period_ticks"), Some(&NbtTag::Int(24000)));

        let tracks = nbt.get_compound("tracks").unwrap();
        let keyframe = |id: &str, index: usize| match &tracks
            .get_compound(id)
            .unwrap()
            .get_list("keyframes")
            .unwrap()[index]
        {
            NbtTag::Compound(fields) => fields.clone(),
            other => panic!("a keyframe must be a compound, got {other:?}"),
        };
        let value = |id: &str, index: usize| keyframe(id, index).get("value").unwrap().clone();

        assert_eq!(
            keyframe("minecraft:visual/sky_light_factor", 0).get("ticks"),
            Some(&NbtTag::Int(730))
        );
        // `Codec.FLOAT` writes a float tag, never a double
        assert_eq!(
            value("minecraft:visual/sky_light_factor", 0),
            NbtTag::Float(1.0)
        );
        // an rgb argument is the hex string `STRING_RGB_COLOR` encodes with
        assert_eq!(
            value("minecraft:visual/sky_color", 0),
            NbtTag::String("#ffffff".to_owned())
        );
        // …while `ColorModifier.ArgbModifier` writes a packed int whenever the
        // argument's alpha is full, and an int tag is not a long tag
        assert_eq!(value("minecraft:visual/cloud_color", 0), NbtTag::Int(-1));
        assert_eq!(
            value("minecraft:visual/sunrise_sunset_color", 0),
            NbtTag::String("#5fefa333".to_owned())
        );
        assert_eq!(
            value("minecraft:gameplay/monsters_burn", 0),
            NbtTag::Byte(0)
        );

        assert_eq!(
            tracks
                .get_compound("minecraft:visual/sun_angle")
                .unwrap()
                .get_compound("ease")
                .unwrap()
                .get_list("cubic_bezier")
                .unwrap(),
            &[
                NbtTag::Float(0.362),
                NbtTag::Float(0.241),
                NbtTag::Float(0.638),
                NbtTag::Float(0.759)
            ]
        );
        assert_eq!(
            tracks
                .get_compound("minecraft:gameplay/cat_waking_up_gift_chance")
                .unwrap()
                .get_string("ease"),
            Some("constant")
        );

        // an opaque payload keeps whatever shape it had, here a string
        let moon =
            mcrs_minecraft_nbt::to_nbt_compound(&NetworkTimeline::from(&timeline("moon.json"))).unwrap();
        assert_eq!(
            moon.get_compound("tracks")
                .unwrap()
                .get_compound("minecraft:visual/moon_phase")
                .unwrap()
                .get_list("keyframes")
                .unwrap()[0]
                .extract_compound()
                .unwrap()
                .get("value"),
            Some(&NbtTag::String("full_moon".to_owned()))
        );

        let markers = nbt.get_compound("time_markers").unwrap();
        assert_eq!(
            markers.get("minecraft:roll_village_siege"),
            Some(&NbtTag::Int(18000))
        );
        assert_eq!(
            markers.get_compound("minecraft:day").unwrap().get("ticks"),
            Some(&NbtTag::Int(1000))
        );
    }

    #[test]
    fn a_marker_with_no_period_reports_no_period_count() {
        let early_game = timeline("early_game.json");
        assert_eq!(early_game.period_ticks, None);
        assert_eq!(early_game.current_ticks(50_000), 50_000);
        assert_eq!(early_game.period_count(50_000), 0);

        let day = timeline("day.json");
        assert_eq!(day.current_ticks(50_000), 2_000);
        assert_eq!(day.period_count(50_000), 2);
        assert_eq!(day.current_ticks(0), 0);
        assert_eq!(day.period_count(23_999), 0);
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

    /// The keyframes of one track, as the sampler reads them.
    fn keys(timeline: &Timeline, id: &str) -> Vec<(f32, [f32; 4])> {
        timeline.tracks[id]
            .keyframes
            .iter()
            .map(|keyframe| (keyframe.ticks as f32, as_channels(&keyframe.value)))
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
            let baked = timeline.bake();
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
        let bake = |ease: Value| {
            bake_one(
                "minecraft:visual/sky_light_factor",
                Some(24000),
                json!({
                    "keyframes": [
                        {"ticks": 0, "value": 0.0},
                        {"ticks": 100, "value": 1.0},
                    ],
                    "modifier": "multiply",
                    "ease": ease,
                }),
            )
        };

        assert_eq!(
            bake(json!("linear")).unwrap().argument.easing(),
            Easing::Linear
        );
        assert_eq!(
            bake(json!("constant")).unwrap().argument.easing(),
            Easing::Constant
        );
        assert!(matches!(
            bake(json!({"cubic_bezier": [0.362, 0.241, 0.638, 0.759]}))
                .unwrap()
                .argument
                .easing(),
            Easing::CubicBezier(_)
        ));

        // a curve the reference registers but we do not implement must not
        // quietly behave as linear
        let err = bake(json!("in_out_bounce")).unwrap_err();
        assert!(
            err.contains("in_out_bounce") && err.contains("not a supported easing"),
            "{err}"
        );
        assert!(bake(json!("nonsense")).is_err());
        assert!(
            bake(json!({"cubic_bezier": [0.5, 0.5, 0.5]})).is_err(),
            "needs four"
        );
        assert!(
            bake(json!({"cubic_bezier": [1.5, 0.0, 0.5, 1.0]})).is_err(),
            "x1 must be in [0; 1]"
        );
        assert!(
            bake(json!({"in_out_bounce": []})).is_err(),
            "not a curve we have"
        );
        assert!(bake(json!(7)).is_err(), "not a curve at all");
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
            let sampler = day.tracks[id].bake(day.period_ticks);
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
        let sun = day.tracks["minecraft:visual/sun_angle"].bake(day.period_ticks);
        let moon = day.tracks["minecraft:visual/moon_angle"].bake(day.period_ticks);

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
        let sun = day.tracks["minecraft:visual/sun_angle"].bake(day.period_ticks);

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
        let sampler = day.tracks[id].bake(day.period_ticks);
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
        let sampler = early.bake().remove(id).unwrap();
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
        let sampler = moon.tracks[id].bake(moon.period_ticks);
        let at = |tick| sampler.sample_argument(tick);
        let phase = |name: &str| AttributeValue::Opaque(json!(name));

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
        let sampler = day.tracks[id].bake(day.period_ticks);
        assert_eq!(sampler.argument.easing(), Easing::Constant);

        // keyframes are 362 → 0.0 and 23667 → 0.7
        assert_eq!(float(&sampler.sample_argument(362)), 0.0);
        assert_eq!(float(&sampler.sample_argument(12_000)), 0.0);
        assert_eq!(float(&sampler.sample_argument(23_666)), 0.0);
        assert_eq!(float(&sampler.sample_argument(23_667)), 0.7);
    }

    #[test]
    fn malformed_tracks_are_rejected_at_load() {
        let track = |keyframes: Vec<(u32, f32)>| {
            json!({
                "keyframes": keyframes
                    .into_iter()
                    .map(|(ticks, value)| json!({"ticks": ticks, "value": value}))
                    .collect::<Vec<_>>(),
                "modifier": "multiply",
            })
        };
        let bake = |keyframes, period| {
            bake_one(
                "minecraft:visual/sky_light_factor",
                period,
                track(keyframes),
            )
        };

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

        let unknown = load(
            "minecraft:visual/sky_colour",
            None,
            json!({"keyframes": [{"ticks": 0, "value": "#ffffff"}]}),
        )
        .unwrap_err();
        assert!(
            unknown.contains("is not an environment attribute"),
            "{unknown}"
        );

        let bad_modifier = load(
            "minecraft:visual/sky_light_factor",
            None,
            json!({"keyframes": [{"ticks": 0, "value": 1.0}], "modifier": "blend"}),
        )
        .unwrap_err();
        assert!(
            bad_modifier.contains("unknown variant `blend`"),
            "{bad_modifier}"
        );

        let wrong_modifier = load(
            "minecraft:visual/sky_light_factor",
            None,
            json!({"keyframes": [{"ticks": 0, "value": true}], "modifier": "or"}),
        )
        .unwrap_err();
        assert!(
            wrong_modifier.contains("Or is not a valid modifier for Float"),
            "{wrong_modifier}"
        );

        let wrong_value = load(
            "minecraft:visual/sky_light_factor",
            None,
            json!({"keyframes": [{"ticks": 0, "value": "noon"}], "modifier": "multiply"}),
        )
        .unwrap_err();
        assert!(
            wrong_value.contains("is not a valid Float value"),
            "{wrong_value}"
        );
    }

    #[test]
    fn a_single_keyframe_track_is_a_constant() {
        for period in [Some(24000), None] {
            let sampler = bake_one(
                "minecraft:visual/sky_light_factor",
                period,
                json!({
                    "keyframes": [{"ticks": 500, "value": 0.25}],
                    "modifier": "multiply",
                }),
            )
            .unwrap();
            for tick in [-1_000_000, -1, 0, 500, 23_999, 1_000_000] {
                assert_eq!(float(&sampler.sample_argument(tick)), 0.25, "at {tick}");
            }
        }
    }
}
