use crate::Weighted;
use mcrs_minecraft_random::Random;
use serde::{Deserialize, Serialize};

/// The entries of a `WeightedList`, whose weights sum above zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    try_from = "Vec<Weighted<T>>",
    into = "Vec<Weighted<T>>",
    bound(
        serialize = "T: Clone + Serialize",
        deserialize = "T: Deserialize<'de>"
    )
)]
pub struct WeightedDistribution<T> {
    pub(crate) first: Box<Weighted<T>>,
    pub(crate) rest: Vec<Weighted<T>>,
}

impl<T> WeightedDistribution<T> {
    fn iter(&self) -> impl Clone + Iterator<Item = &Weighted<T>> {
        std::iter::once(&*self.first).chain(&self.rest)
    }

    pub fn pick<R: Random>(&self, rng: &mut R) -> &T {
        &pick_weighted_by(self.iter(), |entry| entry.weight.0, rng)
            .unwrap_or(&self.first)
            .data
    }
}

impl<T> TryFrom<Vec<Weighted<T>>> for WeightedDistribution<T> {
    type Error = &'static str;

    fn try_from(entries: Vec<Weighted<T>>) -> Result<Self, Self::Error> {
        const EMPTY: &str = "weighted list must contain at least one entry with non-zero weight";
        if entries.iter().map(|entry| entry.weight.0).sum::<i32>() <= 0 {
            return Err(EMPTY);
        }
        let mut entries = entries.into_iter();
        let first = entries.next().ok_or(EMPTY)?;
        Ok(Self {
            first: Box::new(first),
            rest: entries.collect(),
        })
    }
}

impl<T> From<WeightedDistribution<T>> for Vec<Weighted<T>> {
    fn from(distribution: WeightedDistribution<T>) -> Self {
        std::iter::once(*distribution.first)
            .chain(distribution.rest)
            .collect()
    }
}

/// One `nextInt(total)` draw walked through the weights in list order; a total
/// of zero draws nothing.
pub fn pick_weighted_by<'a, T: 'a, R: Random>(
    items: impl IntoIterator<Item = &'a T, IntoIter: Clone>,
    weight: impl Fn(&T) -> i32,
    rng: &mut R,
) -> Option<&'a T> {
    let mut items = items.into_iter();
    let total: i32 = items.clone().map(&weight).sum();
    if total <= 0 {
        return None;
    }
    let mut selection = rng.next_i32_bound(total);
    items.find(|item| {
        selection -= weight(item);
        selection < 0
    })
}
