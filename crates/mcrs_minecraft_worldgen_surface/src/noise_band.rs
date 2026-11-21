use crate::compile::{CompiledCondition, Condition, NoiseId, Op};
use std::iter;

#[derive(Clone, Copy, Debug, PartialEq)]
struct BandEntry {
    min: f64,
    max: f64,
    on_true: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NoiseBand {
    pub(crate) noise: NoiseId,
    pub(crate) is_3d: bool,
    first: u32,
    end: u32,
    otherwise: u32,
}

pub(crate) struct NoiseBands {
    by_guard: Box<[Option<NoiseBand>]>,
    entries: Box<[BandEntry]>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct BandNoise {
    noise: NoiseId,
    is_3d: bool,
}

impl NoiseBands {
    pub(crate) fn compile(tape: &[Op], conditions: &[CompiledCondition]) -> Self {
        let threshold_at = |pc: usize| match tape.get(pc) {
            Some(&Op::Guard { condition, skip_to }) => match conditions
                .get(condition as usize)
                .map(|compiled| &compiled.kind)
            {
                Some(&Condition::NoiseThreshold {
                    noise,
                    min,
                    max,
                    is_3d,
                }) => Some((
                    BandNoise { noise, is_3d },
                    BandEntry {
                        min: min.0,
                        max: max.0,
                        on_true: pc as u32 + 1,
                    },
                    skip_to,
                )),
                _ => None,
            },
            _ => None,
        };
        let mut by_guard = Vec::with_capacity(tape.len());
        let mut tails: Vec<Option<NoiseBand>> = vec![None; tape.len()];
        let mut members = Vec::new();
        let mut entries = Vec::new();
        for pc in 0..tape.len() {
            if let Some(&Some(tail)) = tails.get(pc) {
                by_guard.push((tail.end - tail.first >= 2).then_some(tail));
                continue;
            }
            let Some((head, entry, mut next)) = threshold_at(pc) else {
                by_guard.push(None);
                continue;
            };
            let first = entries.len();
            entries.push(entry);
            members.clear();
            while let Some((noise, entry, skip_to)) = threshold_at(next as usize)
                && noise == head
            {
                members.push(next as usize);
                entries.push(entry);
                next = skip_to;
            }
            if entries.len() - first < 2 {
                entries.truncate(first);
                by_guard.push(None);
                continue;
            }
            let band = NoiseBand {
                noise: head.noise,
                is_3d: head.is_3d,
                first: first as u32,
                end: entries.len() as u32,
                otherwise: next,
            };
            for (at, &member) in (band.first + 1..).zip(&members) {
                if let Some(slot @ None) = tails.get_mut(member) {
                    *slot = Some(NoiseBand { first: at, ..band });
                }
            }
            by_guard.push(Some(band));
        }
        Self {
            by_guard: by_guard.into_boxed_slice(),
            entries: entries.into_boxed_slice(),
        }
    }

    pub(crate) fn at(&self, pc: usize) -> Option<NoiseBand> {
        self.by_guard.get(pc).copied().flatten()
    }

    pub(crate) fn targets(&self, band: NoiseBand) -> impl Iterator<Item = usize> + '_ {
        self.entries
            .get(band.first as usize..band.end as usize)
            .unwrap_or_default()
            .iter()
            .map(|entry| entry.on_true as usize)
            .chain(iter::once(band.otherwise as usize))
    }

    pub(crate) fn select(&self, band: NoiseBand, value: f64) -> usize {
        let entries = self
            .entries
            .get(band.first as usize..band.end as usize)
            .unwrap_or_default();
        let target = entries
            .iter()
            .find(|entry| value >= entry.min && value <= entry.max)
            .map_or(band.otherwise, |entry| entry.on_true);
        target as usize
    }
}
