use serde::de::Error;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Deserialize)]
#[serde(from = "RangeShape<P>")]
pub(crate) enum RangeFile<P> {
    Point(P),
    Bounds { min: Option<P>, max: Option<P> },
}

#[derive(Deserialize)]
#[serde(
    untagged,
    expecting = "a provider, or an object with optional `min` and `max`"
)]
enum RangeShape<P> {
    Bounds {
        min: Option<P>,
        max: Option<P>,
        #[serde(rename = "type", default, deserialize_with = "provider_type")]
        provider_type: (),
    },
    Point(P),
}

impl<P> From<RangeShape<P>> for RangeFile<P> {
    fn from(shape: RangeShape<P>) -> Self {
        match shape {
            RangeShape::Bounds {
                min,
                max,
                provider_type: (),
            } => RangeFile::Bounds { min, max },
            RangeShape::Point(point) => RangeFile::Point(point),
        }
    }
}

fn provider_type<'de, D: Deserializer<'de>>(_: D) -> Result<(), D::Error> {
    Err(D::Error::custom("an object with `type` is a provider"))
}
