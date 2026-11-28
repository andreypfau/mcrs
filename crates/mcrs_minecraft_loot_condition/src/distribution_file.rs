use serde::de::Error;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone)]
pub(crate) struct DistributionFile<P> {
    pub(crate) total: i32,
    pub(crate) entries: Vec<WeightedFile<P>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WeightedFile<P> {
    pub(crate) data: P,
    pub(crate) weight: i32,
}

impl<'de, P: Deserialize<'de>> Deserialize<'de> for DistributionFile<P> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let entries = Vec::<WeightedFile<P>>::deserialize(deserializer)?;
        if entries.iter().any(|entry| entry.weight < 0) {
            return Err(D::Error::custom("a negative weight"));
        }
        let total: i64 = entries.iter().map(|entry| i64::from(entry.weight)).sum();
        let total = i32::try_from(total)
            .ok()
            .filter(|&total| total >= 1)
            .ok_or_else(|| D::Error::custom("the sum of the weights is outside 1..=2147483647"))?;
        Ok(Self { total, entries })
    }
}
