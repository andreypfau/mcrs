use crate::attribute::{AttributeSpec, AttributeValue, Lerp, ModifierError, Operation};

use super::{Easing, Track};

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

impl Track {
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
}
