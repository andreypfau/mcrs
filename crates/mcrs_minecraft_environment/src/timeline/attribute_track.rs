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
///
/// The segments between keyframes, including the wrap-around ones of a periodic timeline, are
/// computed when the track is built and when its period is set, so a read allocates nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    attribute: EnvironmentAttribute,
    modifier: Operation,
    keyframes: Vec<Keyframe>,
    ease: Easing,
    period: Option<u32>,
    segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq)]
struct Segment {
    from: usize,
    to: usize,
    start: i64,
    end: i64,
    ease: Easing,
}

impl Track {
    pub(crate) fn new(
        attribute: EnvironmentAttribute,
        modifier: Operation,
        keyframes: Vec<Keyframe>,
        ease: Easing,
    ) -> Self {
        let mut track = Self {
            attribute,
            modifier,
            keyframes,
            ease,
            period: None,
            segments: Vec::new(),
        };
        track.segments = track.build_segments();
        track
    }

    pub(crate) fn set_period(&mut self, period: Option<u32>) {
        self.period = period;
        self.segments = self.build_segments();
    }

    #[must_use]
    pub fn attribute(&self) -> EnvironmentAttribute {
        self.attribute
    }

    #[must_use]
    pub fn modifier(&self) -> Operation {
        self.modifier
    }

    #[must_use]
    pub fn ease(&self) -> Easing {
        self.ease
    }

    #[must_use]
    pub fn keyframes(&self) -> &[Keyframe] {
        &self.keyframes
    }

    fn build_segments(&self) -> Vec<Segment> {
        let (Some(first), Some(last)) = (self.keyframes.first(), self.keyframes.last()) else {
            return Vec::new();
        };
        if self.keyframes.len() == 1 {
            return vec![Segment {
                from: 0,
                to: 0,
                start: 0,
                end: 0,
                ease: Easing::Named(NamedEasing::Constant),
            }];
        }
        let ease = self.ease;
        let last_index = self.keyframes.len() - 1;
        let segment = |from, to, start, end| Segment {
            from,
            to,
            start,
            end,
            ease,
        };
        let mut segments = Vec::with_capacity(self.keyframes.len() + 1);
        let period = self.period.map(i64::from);
        if let Some(period) = period {
            segments.push(segment(
                last_index,
                0,
                i64::from(last.ticks) - period,
                i64::from(first.ticks),
            ));
        }
        for (at, pair) in self.keyframes.windows(2).enumerate() {
            if let [from, to] = pair {
                segments.push(segment(
                    at,
                    at + 1,
                    i64::from(from.ticks),
                    i64::from(to.ticks),
                ));
            }
        }
        if let Some(period) = period {
            segments.push(segment(
                last_index,
                0,
                i64::from(last.ticks),
                i64::from(first.ticks) + period,
            ));
        }
        segments
    }

    /// The argument of the track at the whole tick `ticks` of its clock.
    #[must_use]
    pub fn sample(&self, ticks: i64) -> Option<Argument> {
        let at = match self.period {
            Some(period) => ticks.rem_euclid(i64::from(period)),
            None => ticks,
        };
        let segment = self
            .segments
            .iter()
            .find(|segment| segment.end > at)
            .or(self.segments.last())?;
        let from = &self.keyframes.get(segment.from)?.argument;
        let to = &self.keyframes.get(segment.to)?.argument;
        if at <= segment.start {
            return Some(from.clone());
        }
        if at >= segment.end {
            return Some(to.clone());
        }
        let x = (at - segment.start) as f32 / (segment.end - segment.start) as f32;
        Some(Argument::interpolate(
            self.modifier,
            info(self.attribute).kind,
            segment.ease.ease(x),
            from,
            to,
        ))
    }

    /// The modifier of the track applied to `subject` at the whole tick `ticks`.
    #[must_use]
    pub fn apply(&self, subject: AttributeValue, ticks: i64) -> AttributeValue {
        let Some(argument) = self.sample(ticks) else {
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
    Ok(Track::new(attribute, modifier, keyframes, ease))
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
