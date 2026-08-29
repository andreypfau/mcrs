use crate::proto::DensityFunctionHolder;
use serde::de::value::MapAccessDeserializer;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::{Hash, Hasher};

/// `Codec.either(Codec.FLOAT, Multipoint)`: a spline is a bare number or an
/// object, at the root and at every point's `value`.
#[derive(Clone, Debug)]
pub enum ProtoSpline {
    Constant(f32),
    Multipoint(Box<ProtoMultipoint>),
}

/// `location`, `derivative` and a constant leaf are plain `Codec.FLOAT`, not
/// `NOISE_VALUE_CODEC`: the ±1e6 bound would reject splines vanilla accepts.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "PointsForm", into = "PointsForm")]
pub struct ProtoMultipoint {
    pub coordinate: DensityFunctionHolder,
    pub locations: Box<[f32]>,
    pub values: Box<[ProtoSpline]>,
    pub derivatives: Box<[f32]>,
}

impl ProtoSpline {
    /// Pre-order over every coordinate density function: the multipoint's own
    /// first, then each point's sub-spline in list order. `domain_axes` and the
    /// compiler's coordinate deduplication both depend on this order.
    pub fn visit_coordinates(&self, f: &mut impl FnMut(&DensityFunctionHolder)) {
        if let ProtoSpline::Multipoint(m) = self {
            f(&m.coordinate);
            for value in &m.values {
                value.visit_coordinates(f);
            }
        }
    }

    pub fn visit_coordinates_mut(&mut self, f: &mut impl FnMut(&mut DensityFunctionHolder)) {
        if let ProtoSpline::Multipoint(m) = self {
            f(&mut m.coordinate);
            for value in m.values.iter_mut() {
                value.visit_coordinates_mut(f);
            }
        }
    }
}

impl PartialEq for ProtoSpline {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Constant(a), Self::Constant(b)) => a.to_bits() == b.to_bits(),
            (Self::Multipoint(a), Self::Multipoint(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for ProtoSpline {}

impl Hash for ProtoSpline {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Constant(v) => {
                0u8.hash(state);
                v.to_bits().hash(state);
            }
            Self::Multipoint(m) => {
                1u8.hash(state);
                m.hash(state);
            }
        }
    }
}

impl PartialEq for ProtoMultipoint {
    fn eq(&self, other: &Self) -> bool {
        self.coordinate == other.coordinate
            && self.values == other.values
            && bits(&self.locations) == bits(&other.locations)
            && bits(&self.derivatives) == bits(&other.derivatives)
    }
}

impl Eq for ProtoMultipoint {}

impl Hash for ProtoMultipoint {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.coordinate.hash(state);
        bits(&self.locations).hash(state);
        self.values.hash(state);
        bits(&self.derivatives).hash(state);
    }
}

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|v| v.to_bits()).collect()
}

impl Serialize for ProtoSpline {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ProtoSpline::Constant(value) => serializer.serialize_f32(*value),
            ProtoSpline::Multipoint(multipoint) => multipoint.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ProtoSpline {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SplineVisitor;

        impl<'de> Visitor<'de> for SplineVisitor {
            type Value = ProtoSpline;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a float or a spline object")
            }

            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<ProtoSpline, E> {
                Ok(ProtoSpline::Constant(value as f32))
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<ProtoSpline, E> {
                Ok(ProtoSpline::Constant(value as f32))
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<ProtoSpline, E> {
                Ok(ProtoSpline::Constant(value as f32))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<ProtoSpline, A::Error> {
                ProtoMultipoint::deserialize(MapAccessDeserializer::new(map))
                    .map(|m| ProtoSpline::Multipoint(Box::new(m)))
            }
        }

        deserializer.deserialize_any(SplineVisitor)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PointsForm {
    coordinate: DensityFunctionHolder,
    points: Vec<PointForm>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PointForm {
    location: f32,
    value: ProtoSpline,
    derivative: f32,
}

impl TryFrom<PointsForm> for ProtoMultipoint {
    type Error = String;

    fn try_from(raw: PointsForm) -> Result<Self, Self::Error> {
        if raw.points.is_empty() {
            return Err("List must have contents".to_string());
        }
        let mut locations = Vec::with_capacity(raw.points.len());
        let mut values = Vec::with_capacity(raw.points.len());
        let mut derivatives = Vec::with_capacity(raw.points.len());
        for point in raw.points {
            locations.push(point.location);
            values.push(point.value);
            derivatives.push(point.derivative);
        }
        Ok(ProtoMultipoint {
            coordinate: raw.coordinate,
            locations: locations.into_boxed_slice(),
            values: values.into_boxed_slice(),
            derivatives: derivatives.into_boxed_slice(),
        })
    }
}

impl From<ProtoMultipoint> for PointsForm {
    fn from(multipoint: ProtoMultipoint) -> Self {
        let ProtoMultipoint {
            coordinate,
            locations,
            values,
            derivatives,
        } = multipoint;
        let points = values
            .into_vec()
            .into_iter()
            .enumerate()
            .map(|(i, value)| PointForm {
                location: locations[i],
                value,
                derivative: derivatives[i],
            })
            .collect();
        PointsForm { coordinate, points }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NESTED: &str = r#"{"coordinate":0.5,"points":[{"location":-1.0,"value":2.0,"derivative":0.0},{"location":1.0,"value":{"coordinate":0.25,"points":[{"location":0.0,"value":3.0,"derivative":1.0}]},"derivative":0.0}]}"#;

    #[test]
    fn the_either_round_trips_both_shapes() {
        let bare: ProtoSpline = serde_json::from_str("1.5").unwrap();
        assert_eq!(serde_json::to_string(&bare).unwrap(), "1.5");

        let nested: ProtoSpline = serde_json::from_str(NESTED).unwrap();
        assert_eq!(serde_json::to_string(&nested).unwrap(), NESTED);
    }

    #[test]
    fn an_empty_point_list_is_a_load_error() {
        serde_json::from_str::<ProtoSpline>(r#"{"coordinate":0.0,"points":[]}"#).unwrap_err();
    }

    /// The compiler deduplicates coordinates on structural equality, so a
    /// derivative that differs only in the sign of zero must stay distinct.
    #[test]
    fn signed_zero_survives_structural_equality() {
        let positive: ProtoSpline = serde_json::from_str(
            r#"{"coordinate":0.0,"points":[{"location":0.0,"value":1.0,"derivative":0.0}]}"#,
        )
        .unwrap();
        let negative: ProtoSpline = serde_json::from_str(
            r#"{"coordinate":0.0,"points":[{"location":0.0,"value":1.0,"derivative":-0.0}]}"#,
        )
        .unwrap();
        assert_ne!(positive, negative);
    }

    #[test]
    fn coordinates_are_visited_pre_order() {
        let nested: ProtoSpline = serde_json::from_str(NESTED).unwrap();
        let mut seen = Vec::new();
        nested.visit_coordinates(&mut |c| seen.push(c.as_constant()));
        assert_eq!(seen, vec![Some(0.5), Some(0.25)]);
    }
}
