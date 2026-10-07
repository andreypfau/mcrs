use std::fmt;

use serde::de::value::MapAccessDeserializer;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

/// A time marker as a timeline declares it: a bare tick count, or an object
/// that also asks for the marker to be offered to commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeMarker {
    pub ticks: u32,
    pub show_in_commands: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FullTimeMarker {
    #[serde(deserialize_with = "super::non_negative_ticks")]
    ticks: u32,
    #[serde(default)]
    show_in_commands: bool,
}

impl From<FullTimeMarker> for TimeMarker {
    fn from(full: FullTimeMarker) -> Self {
        TimeMarker {
            ticks: full.ticks,
            show_in_commands: full.show_in_commands,
        }
    }
}

impl<'de> Deserialize<'de> for TimeMarker {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct MarkerVisitor;

        impl<'de> Visitor<'de> for MarkerVisitor {
            type Value = TimeMarker;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a tick count or an object with `ticks`")
            }

            fn visit_u64<E: de::Error>(self, ticks: u64) -> Result<TimeMarker, E> {
                let ticks = i32::try_from(ticks)
                    .map_err(|_| E::invalid_value(de::Unexpected::Unsigned(ticks), &self))?;
                Ok(TimeMarker {
                    ticks: ticks as u32,
                    show_in_commands: false,
                })
            }

            fn visit_i64<E: de::Error>(self, ticks: i64) -> Result<TimeMarker, E> {
                let unsigned = u64::try_from(ticks)
                    .map_err(|_| E::invalid_type(de::Unexpected::Signed(ticks), &self))?;
                self.visit_u64(unsigned)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<TimeMarker, A::Error> {
                FullTimeMarker::deserialize(MapAccessDeserializer::new(map)).map(TimeMarker::from)
            }
        }

        d.deserialize_any(MarkerVisitor)
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
