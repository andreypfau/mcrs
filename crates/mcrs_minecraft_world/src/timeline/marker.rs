use serde::{Deserialize, Serialize};

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
