use std::collections::BTreeMap;
use std::ops::Deref;

use serde::de::{DeserializeSeed, Error as _, MapAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::attribute::{AttributeSpec, AttributeValue, Operation, RawArgument, attribute};

use super::Easing;

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

    /// `KeyframeTrack.validatePeriod`. The period is a sibling of `tracks`, so
    /// this is the one check the track's own seed cannot make.
    pub(super) fn validate_period(&self, period: u32) -> Result<(), TrackError> {
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
