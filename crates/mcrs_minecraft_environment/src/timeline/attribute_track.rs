use std::collections::BTreeMap;
use std::fmt;

use mcrs_minecraft_nbt::tag::NbtTag;
use serde::de::{self, MapAccess, Visitor};
use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Deserializer, Serialize, Serializer};

use super::easing::{Easing, NamedEasing};
use crate::attribute::{Argument, AttributeValue, Operation, info, read_argument};
use crate::environment_attribute_keys::EnvironmentAttribute;

/// One keyframe of a track: a tick and the argument of the track's modifier there.
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe {
    pub ticks: u32,
    pub argument: Argument,
}

/// How a timeline moves one attribute: a modifier whose argument follows keyframes.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub attribute: EnvironmentAttribute,
    pub modifier: Operation,
    pub keyframes: Vec<Keyframe>,
    pub ease: Easing,
}

struct Segment<'a> {
    from: &'a Keyframe,
    to: &'a Keyframe,
    start: i64,
    end: i64,
    ease: Easing,
}

impl Track {
    fn segments(&self, period: Option<u32>) -> Vec<Segment<'_>> {
        let (Some(first), Some(last)) = (self.keyframes.first(), self.keyframes.last()) else {
            return Vec::new();
        };
        if self.keyframes.len() == 1 {
            return vec![Segment {
                from: first,
                to: first,
                start: 0,
                end: 0,
                ease: Easing::Named(NamedEasing::Constant),
            }];
        }
        let ease = self.ease;
        let segment = |from, to, start, end| Segment {
            from,
            to,
            start,
            end,
            ease,
        };
        let mut segments = Vec::with_capacity(self.keyframes.len() + 1);
        if let Some(period) = period.map(i64::from) {
            segments.push(segment(
                last,
                first,
                i64::from(last.ticks) - period,
                i64::from(first.ticks),
            ));
        }
        segments.extend(self.keyframes.windows(2).filter_map(|pair| match pair {
            [from, to] => Some(segment(
                from,
                to,
                i64::from(from.ticks),
                i64::from(to.ticks),
            )),
            _ => None,
        }));
        if let Some(period) = period.map(i64::from) {
            segments.push(segment(
                last,
                first,
                i64::from(last.ticks),
                i64::from(first.ticks) + period,
            ));
        }
        segments
    }

    /// The argument of the track at the whole tick `ticks` of its clock.
    #[must_use]
    pub fn sample(&self, ticks: i64, period: Option<u32>) -> Option<Argument> {
        let at = match period {
            Some(period) => ticks.rem_euclid(i64::from(period)),
            None => ticks,
        };
        let segments = self.segments(period);
        let segment = segments
            .iter()
            .find(|segment| segment.end > at)
            .or(segments.last())?;
        if at <= segment.start {
            return Some(segment.from.argument.clone());
        }
        if at >= segment.end {
            return Some(segment.to.argument.clone());
        }
        let x = (at - segment.start) as f32 / (segment.end - segment.start) as f32;
        Some(Argument::interpolate(
            self.modifier,
            info(self.attribute).kind,
            segment.ease.ease(x),
            &segment.from.argument,
            &segment.to.argument,
        ))
    }

    /// The modifier of the track applied to `subject` at the whole tick `ticks`.
    #[must_use]
    pub fn apply(
        &self,
        subject: AttributeValue,
        ticks: i64,
        period: Option<u32>,
    ) -> AttributeValue {
        let Some(argument) = self.sample(ticks, period) else {
            return subject;
        };
        match self.modifier.apply(&subject, &argument) {
            Some(value) => value,
            None => subject,
        }
    }

    /// The load fails for a track whose keyframes do not fit `period`.
    pub(crate) fn check(&self, period: Option<u32>) -> Result<(), String> {
        let ticks: Vec<u32> = self
            .keyframes
            .iter()
            .map(|keyframe| keyframe.ticks)
            .collect();
        let (Some(first), Some(last)) = (ticks.first(), ticks.last()) else {
            return Err("a track has no keyframe".to_owned());
        };
        if ticks.windows(2).any(|pair| pair.first() > pair.get(1)) {
            return Err("the keyframes are not in order of ticks".to_owned());
        }
        if first == last && ticks.len() >= 3 {
            return Err("three or more keyframes share the only tick".to_owned());
        }
        if ticks
            .windows(4)
            .any(|run| run.iter().all(|tick| Some(tick) == run.first()))
        {
            return Err("four or more keyframes in a row share a tick".to_owned());
        }
        if let Some(period) = period
            && let Some(outside) = ticks.iter().find(|tick| **tick > period)
        {
            return Err(format!(
                "the keyframe at {outside} lies outside the period {period}"
            ));
        }
        Ok(())
    }
}

fn read_track(attribute: EnvironmentAttribute, tag: &NbtTag) -> Result<Track, String> {
    let NbtTag::Compound(fields) = tag else {
        return Err("a track is an object".to_owned());
    };
    let modifier: Operation = match fields.get("modifier") {
        Some(NbtTag::String(name)) => name.parse()?,
        Some(_) => return Err("`modifier` is not a string".to_owned()),
        None => Operation::Override,
    };
    let ease = match fields.get("ease") {
        Some(ease) => Easing::read(ease)?,
        None => Easing::default(),
    };
    let Some(NbtTag::List(items)) = fields.get("keyframes") else {
        return Err("missing `keyframes`".to_owned());
    };
    let mut keyframes = Vec::with_capacity(items.len());
    for (at, item) in items.iter().enumerate() {
        let read = || {
            let NbtTag::Compound(keyframe) = item else {
                return Err("a keyframe is an object".to_owned());
            };
            let ticks = match keyframe.get("ticks") {
                Some(NbtTag::Byte(ticks)) => i64::from(*ticks),
                Some(NbtTag::Short(ticks)) => i64::from(*ticks),
                Some(NbtTag::Int(ticks)) => i64::from(*ticks),
                _ => return Err("`ticks` is not an int".to_owned()),
            };
            let ticks = u32::try_from(ticks).map_err(|_| format!("`ticks` {ticks} is negative"))?;
            let value = keyframe.get("value").ok_or("missing `value`")?;
            Ok(Keyframe {
                ticks,
                argument: read_argument(attribute, modifier, value)?,
            })
        };
        keyframes.push(read().map_err(|message: String| format!("keyframes[{at}]: {message}"))?);
    }
    Ok(Track {
        attribute,
        modifier,
        keyframes,
        ease,
    })
}

struct KeyframeForm<'a>(&'a Keyframe);

impl Serialize for KeyframeForm<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut keyframe = serializer.serialize_struct("Keyframe", 2)?;
        keyframe.serialize_field("ticks", &self.0.ticks)?;
        keyframe.serialize_field("value", &self.0.argument)?;
        keyframe.end()
    }
}

impl Serialize for Track {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let linear = self.ease == Easing::default();
        let overrides = self.modifier == Operation::Override;
        let fields = 1 + usize::from(!linear) + usize::from(!overrides);
        let mut map = serializer.serialize_map(Some(fields))?;
        if !linear {
            map.serialize_entry("ease", &self.ease)?;
        }
        map.serialize_entry(
            "keyframes",
            &self.keyframes.iter().map(KeyframeForm).collect::<Vec<_>>(),
        )?;
        if !overrides {
            map.serialize_entry("modifier", self.modifier.name())?;
        }
        map.end()
    }
}

pub(crate) fn serialize_tracks<S: Serializer>(
    tracks: &BTreeMap<EnvironmentAttribute, Track>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    // The files of the game list the tracks in the order of their attribute ids.
    let mut entries: Vec<(&EnvironmentAttribute, &Track)> = tracks.iter().collect();
    entries.sort_by_key(|(attribute, _)| attribute.as_static_str());
    serializer.collect_map(entries)
}

pub(crate) fn deserialize_tracks<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<EnvironmentAttribute, Track>, D::Error> {
    struct TracksVisitor;

    impl<'de> Visitor<'de> for TracksVisitor {
        type Value = BTreeMap<EnvironmentAttribute, Track>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a map from environment attribute ids to tracks")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut tracks = BTreeMap::new();
            while let Some(attribute) = map.next_key::<EnvironmentAttribute>()? {
                let tag: NbtTag = map.next_value()?;
                let track = read_track(attribute, &tag).map_err(|message| {
                    de::Error::custom(format!("{}: {message}", attribute.as_static_str()))
                })?;
                tracks.insert(attribute, track);
            }
            Ok(tracks)
        }
    }

    deserializer.deserialize_map(TracksVisitor)
}
