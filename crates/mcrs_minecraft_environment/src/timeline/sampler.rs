use crate::attribute::{AttributeSpec, AttributeValue, ModifierError, Operation};

use super::Track;

/// A track's keyframes, sampled by plain linear interpolation between the two
/// keyframes around a tick. A periodic track wraps from its last keyframe to
/// its first.
#[derive(Debug, Clone)]
pub struct TrackSampler {
    period_ticks: Option<u32>,
    keyframes: Vec<(f64, AttributeValue)>,
}

impl TrackSampler {
    pub fn sample(&self, ticks: i64) -> AttributeValue {
        self.sample_at(ticks as f64)
    }

    pub fn sample_at(&self, ticks: f64) -> AttributeValue {
        let (first, last) = (
            &self.keyframes[0],
            &self.keyframes[self.keyframes.len() - 1],
        );
        let Some(period) = self.period_ticks.map(f64::from) else {
            if ticks <= first.0 {
                return first.1.clone();
            }
            if ticks >= last.0 {
                return last.1.clone();
            }
            let next = self
                .keyframes
                .iter()
                .position(|(at, _)| *at > ticks)
                .unwrap();
            return between(&self.keyframes[next - 1], &self.keyframes[next], ticks);
        };
        let ticks = ticks.rem_euclid(period);
        match self.keyframes.iter().position(|(at, _)| *at > ticks) {
            Some(0) => between(&(last.0 - period, last.1.clone()), first, ticks),
            Some(next) => between(&self.keyframes[next - 1], &self.keyframes[next], ticks),
            None => between(last, &(first.0 + period, first.1.clone()), ticks),
        }
    }
}

fn between(from: &(f64, AttributeValue), to: &(f64, AttributeValue), ticks: f64) -> AttributeValue {
    if to.0 <= from.0 {
        return from.1.clone();
    }
    let alpha = ((ticks - from.0) / (to.0 - from.0)).clamp(0.0, 1.0) as f32;
    match (&from.1, &to.1) {
        (AttributeValue::Float(a), AttributeValue::Float(b)) => {
            AttributeValue::Float(a + (b - a) * alpha)
        }
        (AttributeValue::Color(a), AttributeValue::Color(b)) => {
            let channel = |shift: u32| {
                let (a, b) = (((a >> shift) & 0xFF) as f32, ((b >> shift) & 0xFF) as f32);
                ((a + (b - a) * alpha).round() as u32) << shift
            };
            AttributeValue::Color(channel(24) | channel(16) | channel(8) | channel(0))
        }
        (value, _) => value.clone(),
    }
}

/// A track together with the modifier whose argument its keyframes hold.
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
        crate::attribute::apply(self.modifier, base, &self.argument.sample_at(ticks))
    }
}

impl Track {
    pub fn bake(&self, period_ticks: Option<u32>) -> AttributeTrackSampler {
        AttributeTrackSampler {
            attribute: self.attribute,
            modifier: self.operation(),
            argument: TrackSampler {
                period_ticks,
                keyframes: self
                    .keyframes
                    .iter()
                    .map(|keyframe| (f64::from(keyframe.ticks), keyframe.value.clone()))
                    .collect(),
            },
        }
    }
}
