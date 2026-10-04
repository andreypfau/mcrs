use std::collections::BTreeMap;
use std::ops::Deref;

use serde::de::{DeserializeSeed, Error as _, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::attribute::spec::{Draft, DraftSeed};
use crate::attribute::{ArgumentRef, AttributeSpec, AttributeValue, Operation, attribute};

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
/// value, so a keyframe is read in the type the (attribute, modifier) pair
/// selects rather than in whatever shape the source happened to have.
///
/// `modifier` may follow `keyframes`, so each keyframe is read as far as the
/// attribute's type determines it and finished once the modifier is known.
struct TrackSeed(&'static AttributeSpec);

impl<'de> DeserializeSeed<'de> for TrackSeed {
    type Value = Track;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Track, D::Error> {
        d.deserialize_map(TrackVisitor(self.0))
    }
}

struct TrackVisitor(&'static AttributeSpec);

impl<'de> Visitor<'de> for TrackVisitor {
    type Value = Track;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a track of keyframes, with an optional modifier and ease")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Track, A::Error> {
        const FIELDS: &[&str] = &["keyframes", "modifier", "ease"];

        let spec = self.0;
        let mut drafts: Option<Vec<(u32, Draft)>> = None;
        let mut modifier: Option<Operation> = None;
        let mut ease: Option<Easing> = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "keyframes" if drafts.is_none() => {
                    drafts = Some(map.next_value_seed(KeyframesSeed(spec))?);
                }
                "modifier" if modifier.is_none() => modifier = map.next_value()?,
                "ease" if ease.is_none() => ease = map.next_value()?,
                "keyframes" => return Err(A::Error::duplicate_field("keyframes")),
                "modifier" => return Err(A::Error::duplicate_field("modifier")),
                "ease" => return Err(A::Error::duplicate_field("ease")),
                other => return Err(A::Error::unknown_field(other, FIELDS)),
            }
        }
        let drafts = drafts.ok_or_else(|| A::Error::missing_field("keyframes"))?;

        let operation = modifier.unwrap_or(Operation::Override);
        let keyframes = drafts
            .into_iter()
            .map(|(ticks, draft)| {
                Ok(Keyframe {
                    ticks,
                    value: draft.finish(spec, operation).map_err(A::Error::custom)?,
                })
            })
            .collect::<Result<Vec<_>, A::Error>>()?;
        validate_keyframes(&keyframes).map_err(A::Error::custom)?;

        Ok(Track {
            attribute: spec,
            keyframes,
            modifier,
            ease,
        })
    }
}

struct KeyframesSeed(&'static AttributeSpec);

impl<'de> DeserializeSeed<'de> for KeyframesSeed {
    type Value = Vec<(u32, Draft)>;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for KeyframesSeed {
    type Value = Vec<(u32, Draft)>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a list of keyframes")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut drafts = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(1024));
        while let Some(keyframe) = seq.next_element_seed(KeyframeSeed(self.0))? {
            drafts.push(keyframe);
        }
        Ok(drafts)
    }
}

struct KeyframeSeed(&'static AttributeSpec);

impl<'de> DeserializeSeed<'de> for KeyframeSeed {
    type Value = (u32, Draft);

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for KeyframeSeed {
    type Value = (u32, Draft);

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a keyframe of ticks and value")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        const FIELDS: &[&str] = &["ticks", "value"];

        let (mut ticks, mut value) = (None, None);
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "ticks" if ticks.is_none() => ticks = Some(map.next_value::<u32>()?),
                "value" if value.is_none() => {
                    value = Some(map.next_value_seed(DraftSeed(self.0))?);
                }
                "ticks" => return Err(A::Error::duplicate_field("ticks")),
                "value" => return Err(A::Error::duplicate_field("value")),
                other => return Err(A::Error::unknown_field(other, FIELDS)),
            }
        }
        Ok((
            ticks.ok_or_else(|| A::Error::missing_field("ticks"))?,
            value.ok_or_else(|| A::Error::missing_field("value"))?,
        ))
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
            &ArgumentRef {
                spec: self.track.attribute,
                op: self.track.operation(),
                value: &self.keyframe.value,
            },
        )?;
        map.end()
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
