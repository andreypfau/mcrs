use std::collections::BTreeMap;
use std::sync::Arc;

use mcrs_minecraft_core::codec::{NonNegativeInt, PositiveInt};
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::{ResourceLocation, rl};
use mcrs_minecraft_registry::Id;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::world_clock::WorldClock;

mod easing;
mod marker;
mod sampler;
mod track;

pub use easing::{CubicBezier, Easing};
pub use marker::TimeMarker;
pub use sampler::{AttributeTrackSampler, TrackSampler};
pub use track::{Keyframe, Track, TrackError, Tracks};

pub type TimeMarkers = BTreeMap<ResourceLocation<Arc<str>>, TimeMarker>;

fn non_negative_ticks<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    NonNegativeInt::deserialize(d).map(|ticks| ticks.0 as u32)
}

#[derive(Debug, Clone, Serialize)]
pub struct Timeline {
    pub clock: Id<WorldClock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_ticks: Option<u32>,
    #[serde(skip_serializing_if = "no_tracks")]
    pub tracks: Tracks,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub time_markers: TimeMarkers,
}

fn no_tracks(tracks: &Tracks) -> bool {
    tracks.is_empty()
}

impl RegistryKey for Timeline {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:timeline");
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TimelineRepr {
    clock: Id<WorldClock>,
    #[serde(default)]
    period_ticks: Option<PositiveInt>,
    #[serde(default)]
    tracks: Tracks,
    #[serde(default)]
    time_markers: TimeMarkers,
}

impl<'de> Deserialize<'de> for Timeline {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let repr = TimelineRepr::deserialize(d)?;
        let period_ticks = repr.period_ticks.map(|period| period.0 as u32);
        if let Some(period) = period_ticks {
            for (marker, info) in &repr.time_markers {
                // Exclusive at the top, unlike the inclusive keyframe bound: a
                // marker on the period boundary would occur twice a period.
                if info.ticks >= period {
                    return Err(D::Error::custom(TimelineError::MarkerOutsidePeriod {
                        marker: marker.clone(),
                        ticks: info.ticks,
                        period,
                    }));
                }
            }
            for (id, track) in repr.tracks.iter() {
                track.validate_period(period).map_err(|kind| {
                    D::Error::custom(TimelineError::Track {
                        track: (*id).to_owned(),
                        kind,
                    })
                })?;
            }
        }
        Ok(Timeline {
            clock: repr.clock,
            period_ticks,
            tracks: repr.tracks,
            time_markers: repr.time_markers,
        })
    }
}

/// A timeline as the client receives it: only the tracks it is allowed to see.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct NetworkTimeline(Timeline);

impl From<&Timeline> for NetworkTimeline {
    fn from(timeline: &Timeline) -> Self {
        NetworkTimeline(Timeline {
            clock: timeline.clock,
            period_ticks: timeline.period_ticks,
            tracks: timeline.tracks.syncable(),
            time_markers: timeline.time_markers.clone(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TimelineError {
    #[error("time marker `{marker}` at tick {ticks} must be in range [0; {period})")]
    MarkerOutsidePeriod {
        marker: ResourceLocation<Arc<str>>,
        ticks: u32,
        period: u32,
    },
    #[error("timeline track `{track}`: {kind}")]
    Track {
        track: String,
        #[source]
        kind: TrackError,
    },
}

impl Timeline {
    /// Bake every track of this timeline.
    ///
    /// Derived once from the loaded timeline and never invalidated, so the result
    /// is worth keeping; the samplers it holds retain nothing themselves.
    pub fn bake(&self) -> BTreeMap<&'static str, AttributeTrackSampler> {
        self.tracks
            .iter()
            .map(|(id, track)| (*id, track.bake(self.period_ticks)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute::{AttributeValue, MoonPhase, Operation};
    use crate::world_clock::TEST_CLOCKS;
    use mcrs_minecraft_nbt::tag::NbtTag;
    use mcrs_minecraft_worldgen_testing::{assets_dir, reencode};
    use serde_json::{Value, json};

    const SHIPPED: [&str; 4] = [
        "day.json",
        "moon.json",
        "villager_schedule.json",
        "early_game.json",
    ];

    fn raw(name: &str) -> Value {
        let bytes = std::fs::read(assets_dir().join("minecraft/timeline").join(name)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn timeline(name: &str) -> Timeline {
        TEST_CLOCKS.scope(|| serde_json::from_value(raw(name)).unwrap())
    }

    /// A one-track timeline read the way a datapack reaches one.
    fn load(attribute: &str, period_ticks: Option<u32>, track: Value) -> Result<Timeline, String> {
        let mut doc = json!({"clock": "minecraft:overworld", "tracks": {attribute: track}});
        if let Some(period) = period_ticks {
            doc["period_ticks"] = json!(period);
        }
        TEST_CLOCKS.scope(|| serde_json::from_value(doc).map_err(|e| e.to_string()))
    }

    fn bake_one(
        attribute: &str,
        period_ticks: Option<u32>,
        track: Value,
    ) -> Result<AttributeTrackSampler, String> {
        let timeline = load(attribute, period_ticks, track)?;
        Ok(timeline.tracks[attribute].bake(timeline.period_ticks))
    }

    fn written(timeline: &Timeline) -> Value {
        TEST_CLOCKS.scope(|| reencode(timeline))
    }

    fn sent(timeline: &Timeline) -> Value {
        TEST_CLOCKS.scope(|| reencode(&NetworkTimeline::from(timeline)))
    }

    #[test]
    fn the_network_timeline_drops_unsyncable_tracks() {
        let one_of_each: Timeline = read_document(json!({
            "clock": "minecraft:overworld",
            "tracks": {
                "minecraft:visual/sky_light_factor":
                    {"keyframes": [{"ticks": 0, "value": 1.0}]},
                "minecraft:gameplay/monsters_burn":
                    {"keyframes": [{"ticks": 0, "value": true}]},
            },
        }))
        .unwrap();
        let tracks = &sent(&one_of_each)["tracks"];
        let kept: Vec<&str> = tracks
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(kept, ["minecraft:visual/sky_light_factor"]);

        let only_server_side: Timeline = read_document(json!({
            "clock": "minecraft:overworld",
            "tracks": {
                "minecraft:gameplay/monsters_burn":
                    {"keyframes": [{"ticks": 0, "value": true}]},
            },
        }))
        .unwrap();
        assert!(sent(&only_server_side).get("tracks").is_none());
    }

    #[test]
    fn tick_counts_outside_the_games_int_ranges_are_refused() {
        let past_int = i64::from(i32::MAX) + 1;
        let track = |ticks: Value| json!({"minecraft:visual/sky_light_factor": {"keyframes": [{"ticks": ticks, "value": 1.0}]}});
        for (field, document) in [
            ("period_ticks", json!({"period_ticks": 0})),
            ("period_ticks", json!({"period_ticks": past_int})),
            ("keyframe ticks", json!({"tracks": track(json!(-1))})),
            ("keyframe ticks", json!({"tracks": track(json!(past_int))})),
            (
                "bare marker",
                json!({"time_markers": {"minecraft:noon": past_int}}),
            ),
            (
                "marker ticks",
                json!({"time_markers": {"minecraft:noon": {"ticks": past_int}}}),
            ),
            (
                "marker ticks",
                json!({"time_markers": {"minecraft:noon": {"ticks": -1}}}),
            ),
        ] {
            let mut document = document;
            document["clock"] = json!("minecraft:overworld");
            let read = TEST_CLOCKS.scope(|| serde_json::from_value::<Timeline>(document.clone()));
            assert!(read.is_err(), "{field}: {document} loads");
        }
        let edge = TEST_CLOCKS.scope(|| {
            serde_json::from_value::<Timeline>(json!({
                "clock": "minecraft:overworld",
                "time_markers": {"minecraft:noon": i32::MAX},
                "tracks": track(json!(i32::MAX)),
            }))
        });
        assert!(edge.is_ok(), "{edge:?}");
    }

    #[test]
    fn every_shipped_timeline_survives_the_round_trip_unchanged() {
        for name in SHIPPED {
            assert_eq!(written(&timeline(name)), raw(name), "{name}");
        }
    }

    #[test]
    fn the_network_form_of_a_shipped_timeline_is_the_file_minus_unsyncable_tracks() {
        for name in SHIPPED {
            let mut expected = raw(name);
            let tracks = expected["tracks"].as_object_mut().unwrap();
            tracks.retain(|id, _| crate::attribute::is_syncable(id));
            if tracks.is_empty() {
                expected.as_object_mut().unwrap().remove("tracks");
            }
            assert_eq!(sent(&timeline(name)), expected, "{name}");
        }
    }

    #[test]
    fn both_easing_shapes_and_both_marker_shapes_survive_the_round_trip() {
        let day = timeline("day.json");

        assert_eq!(
            day.tracks["minecraft:visual/sun_angle"].ease,
            Some(Easing::CubicBezier(
                CubicBezier::new(0.362, 0.241, 0.638, 0.759).unwrap()
            ))
        );
        assert_eq!(
            day.tracks["minecraft:gameplay/cat_waking_up_gift_chance"].ease,
            Some(Easing::Constant)
        );
        assert_eq!(day.tracks["minecraft:visual/sky_color"].ease, None);
        assert_eq!(
            day.time_markers["minecraft:day"],
            TimeMarker {
                ticks: 1000,
                show_in_commands: true
            }
        );
        assert_eq!(
            day.time_markers["minecraft:roll_village_siege"],
            TimeMarker {
                ticks: 18000,
                show_in_commands: false
            }
        );

        let sent = written(&day);
        // the object easing keeps its four controls, the named one stays a name
        assert_eq!(
            sent["tracks"]["minecraft:visual/sun_angle"]["ease"],
            json!({"cubic_bezier": [0.362, 0.241, 0.638, 0.759]})
        );
        assert_eq!(
            sent["tracks"]["minecraft:gameplay/cat_waking_up_gift_chance"]["ease"],
            json!("constant")
        );
        assert!(
            sent["tracks"]["minecraft:visual/sky_color"]
                .get("ease")
                .is_none()
        );
        // an absent modifier stays absent rather than being written as `override`
        assert!(
            sent["tracks"]["minecraft:visual/sun_angle"]
                .get("modifier")
                .is_none()
        );

        assert_eq!(sent["time_markers"], raw("day.json")["time_markers"]);
        assert!(sent["time_markers"]["minecraft:roll_village_siege"].is_number());
        assert!(sent["time_markers"]["minecraft:day"].is_object());
    }

    #[test]
    fn the_nbt_form_types_every_keyframe_the_way_the_client_reads_it() {
        let day = timeline("day.json");
        let nbt = TEST_CLOCKS.scope(|| mcrs_minecraft_nbt::to_nbt_compound(&day).unwrap());

        assert_eq!(nbt.get_string("clock"), Some("minecraft:overworld"));
        assert_eq!(nbt.get("period_ticks"), Some(&NbtTag::Int(24000)));

        let tracks = nbt.get_compound("tracks").unwrap();
        let keyframe = |id: &str, index: usize| match &tracks
            .get_compound(id)
            .unwrap()
            .get_list("keyframes")
            .unwrap()[index]
        {
            NbtTag::Compound(fields) => fields.clone(),
            other => panic!("a keyframe must be a compound, got {other:?}"),
        };
        let value = |id: &str, index: usize| keyframe(id, index).get("value").unwrap().clone();

        assert_eq!(
            keyframe("minecraft:visual/sky_light_factor", 0).get("ticks"),
            Some(&NbtTag::Int(730))
        );
        // `Codec.FLOAT` writes a float tag, never a double
        assert_eq!(
            value("minecraft:visual/sky_light_factor", 0),
            NbtTag::Float(1.0)
        );
        // an rgb argument is the hex string `STRING_RGB_COLOR` encodes with
        assert_eq!(
            value("minecraft:visual/sky_color", 0),
            NbtTag::String("#ffffff".to_owned())
        );
        // `ColorModifier.ArgbModifier` decodes a packed int but always encodes hex
        assert_eq!(
            value("minecraft:visual/cloud_color", 0),
            NbtTag::String("#ffffffff".to_owned())
        );
        assert_eq!(
            value("minecraft:visual/sunrise_sunset_color", 0),
            NbtTag::String("#5fefa333".to_owned())
        );
        assert_eq!(
            value("minecraft:gameplay/monsters_burn", 0),
            NbtTag::Byte(0)
        );

        assert_eq!(
            tracks
                .get_compound("minecraft:visual/sun_angle")
                .unwrap()
                .get_compound("ease")
                .unwrap()
                .get_list("cubic_bezier")
                .unwrap(),
            &[
                NbtTag::Float(0.362),
                NbtTag::Float(0.241),
                NbtTag::Float(0.638),
                NbtTag::Float(0.759)
            ]
        );
        assert_eq!(
            tracks
                .get_compound("minecraft:gameplay/cat_waking_up_gift_chance")
                .unwrap()
                .get_string("ease"),
            Some("constant")
        );

        // an opaque payload keeps whatever shape it had, here a string
        let moon = TEST_CLOCKS
            .scope(|| mcrs_minecraft_nbt::to_nbt_compound(&timeline("moon.json")).unwrap());
        assert_eq!(
            moon.get_compound("tracks")
                .unwrap()
                .get_compound("minecraft:visual/moon_phase")
                .unwrap()
                .get_list("keyframes")
                .unwrap()[0]
                .extract_compound()
                .unwrap()
                .get("value"),
            Some(&NbtTag::String("full_moon".to_owned()))
        );

        let markers = nbt.get_compound("time_markers").unwrap();
        assert_eq!(
            markers.get("minecraft:roll_village_siege"),
            Some(&NbtTag::Int(18000))
        );
        assert_eq!(
            markers.get_compound("minecraft:day").unwrap().get("ticks"),
            Some(&NbtTag::Int(1000))
        );
    }

    // ── Baking and sampling ──────────────────────────────────────────────────

    const DAY: f32 = 24000.0;

    fn float(value: &AttributeValue) -> f32 {
        match value {
            AttributeValue::Float(v) => *v,
            other => panic!("expected a float, got {other:?}"),
        }
    }

    fn channels(color: u32) -> [f32; 4] {
        [24, 16, 8, 0].map(|shift| (color >> shift & 0xFF) as f32)
    }

    /// Whatever an argument parses to, as four channels: a colour's bytes, or a
    /// float in the first slot. Lets one oracle cover both kinds of track.
    fn as_channels(value: &AttributeValue) -> [f32; 4] {
        match value {
            AttributeValue::Float(v) => [*v, 0.0, 0.0, 0.0],
            AttributeValue::Color(c) => channels(*c),
            other => panic!("expected a float or a colour, got {other:?}"),
        }
    }

    /// The keyframes of one track, as the sampler reads them.
    fn keys(timeline: &Timeline, id: &str) -> Vec<(f32, [f32; 4])> {
        timeline.tracks[id]
            .keyframes
            .iter()
            .map(|keyframe| (keyframe.ticks as f32, as_channels(&keyframe.value)))
            .collect()
    }

    /// The wrap-and-lerp of the hand-extracted reference tracks, in float space,
    /// as an oracle independent of the baked segment list.
    fn daylight_track(keys: &[(f32, [f32; 4])], ticks: f32) -> [f32; 4] {
        let mut at = ticks.rem_euclid(DAY);
        if at < keys[0].0 {
            at += DAY;
        }
        for (index, &(start, value)) in keys.iter().enumerate() {
            let (mut end, next) = keys[(index + 1) % keys.len()];
            if index + 1 == keys.len() {
                end += DAY;
            }
            if at <= end {
                let fraction = (at - start) / (end - start);
                let mut blended = [0.0; 4];
                for channel in 0..4 {
                    blended[channel] = value[channel] * (1.0 - fraction) + next[channel] * fraction;
                }
                return blended;
            }
        }
        keys[0].1
    }

    /// `daylight.rs::sun_angle`, in degrees: vanilla's old closed-form
    /// approximation of the cubic bezier the track now names.
    fn daylight_sun_angle(ticks: f32) -> f32 {
        let day = (ticks / DAY - 0.25).rem_euclid(1.0);
        let eased = 0.5 - (day * std::f32::consts::PI).cos() * 0.5;
        (day * 2.0 + eased) / 3.0 * 360.0
    }

    #[test]
    fn every_shipped_track_bakes_and_samples() {
        let expected = [
            ("day.json", 18, Some(24000)),
            ("moon.json", 2, Some(192000)),
            ("villager_schedule.json", 2, Some(24000)),
            ("early_game.json", 1, None),
        ];
        for (name, tracks, period) in expected {
            let timeline = timeline(name);
            assert_eq!(timeline.period_ticks, period, "{name} period");
            let baked = timeline.bake();
            assert_eq!(baked.len(), tracks, "{name} track count");

            for (id, sampler) in &baked {
                for ticks in (-50000..250000).step_by(997) {
                    let sampled = sampler.sample_argument(ticks);
                    assert_eq!(
                        sampled,
                        sampler.sample_argument(ticks),
                        "{name} / {id} at {ticks} is not a pure function of ticks"
                    );
                }
            }
        }
    }

    #[test]
    fn only_the_easings_the_assets_use_are_implemented() {
        let bake = |ease: Value| {
            bake_one(
                "minecraft:visual/sky_light_factor",
                Some(24000),
                json!({
                    "keyframes": [
                        {"ticks": 0, "value": 0.0},
                        {"ticks": 100, "value": 1.0},
                    ],
                    "modifier": "multiply",
                    "ease": ease,
                }),
            )
        };

        assert_eq!(
            bake(json!("linear")).unwrap().argument.easing(),
            Easing::Linear
        );
        assert_eq!(
            bake(json!("constant")).unwrap().argument.easing(),
            Easing::Constant
        );
        assert!(matches!(
            bake(json!({"cubic_bezier": [0.362, 0.241, 0.638, 0.759]}))
                .unwrap()
                .argument
                .easing(),
            Easing::CubicBezier(_)
        ));

        // a curve the reference registers but we do not implement must not
        // quietly behave as linear
        let err = bake(json!("in_out_bounce")).unwrap_err();
        assert!(
            err.contains("in_out_bounce") && err.contains("not a supported easing"),
            "{err}"
        );
        assert!(bake(json!("nonsense")).is_err());
        assert!(
            bake(json!({"cubic_bezier": [0.5, 0.5, 0.5]})).is_err(),
            "needs four"
        );
        assert!(
            bake(json!({"cubic_bezier": [1.5, 0.0, 0.5, 1.0]})).is_err(),
            "x1 must be in [0; 1]"
        );
        assert!(
            bake(json!({"in_out_bounce": []})).is_err(),
            "not a curve we have"
        );
        assert!(bake(json!(7)).is_err(), "not a curve at all");
    }

    #[test]
    fn the_daylight_tables_are_the_day_json_tracks() {
        // hand-extracted from `timeline/day.json`
        const SKY_LIGHT_FACTOR: [(f32, f32); 4] = [
            (730.0, 1.0),
            (11270.0, 1.0),
            (13140.0, 0.24),
            (22860.0, 0.24),
        ];
        const STAR_BRIGHTNESS: [(f32, f32); 12] = [
            (92.0, 0.037),
            (627.0, 0.0),
            (11373.0, 0.0),
            (11732.0, 0.016),
            (11959.0, 0.044),
            (12399.0, 0.143),
            (12729.0, 0.258),
            (13228.0, 0.5),
            (22772.0, 0.5),
            (23032.0, 0.364),
            (23356.0, 0.225),
            (23758.0, 0.101),
        ];
        // the multiply factor daylight.rs writes as a scalar where day.json
        // writes #ffffff and #000000
        const SKY_COLOR: [(f32, f32); 4] =
            [(133.0, 1.0), (11867.0, 1.0), (13670.0, 0.0), (22330.0, 0.0)];

        let day = timeline("day.json");
        for (id, table) in [
            ("minecraft:visual/sky_light_factor", &SKY_LIGHT_FACTOR[..]),
            ("minecraft:visual/star_brightness", &STAR_BRIGHTNESS[..]),
        ] {
            let baked: Vec<_> = keys(&day, id).iter().map(|&(t, v)| (t, v[0])).collect();
            assert_eq!(baked, table, "{id} does not match its daylight.rs table");
        }

        let sky_color: Vec<_> = keys(&day, "minecraft:visual/sky_color")
            .iter()
            .map(|&(t, v)| (t, v[1] / 255.0))
            .collect();
        assert_eq!(sky_color, SKY_COLOR);
    }

    #[test]
    fn sampling_matches_the_daylight_tables_at_and_between_their_keys() {
        let day = timeline("day.json");
        let interpolated = [
            "minecraft:gameplay/sky_light_level",
            "minecraft:visual/cloud_color",
            "minecraft:visual/fog_color",
            "minecraft:visual/sky_color",
            "minecraft:visual/sky_light_color",
            "minecraft:visual/sky_light_factor",
            "minecraft:visual/star_brightness",
            "minecraft:visual/sunrise_sunset_color",
        ];

        for id in interpolated {
            let table = keys(&day, id);
            let sampler = day.tracks[id].bake(day.period_ticks);
            let is_color = matches!(sampler.sample_argument(0), AttributeValue::Color(_));

            let ticks = table
                .iter()
                .map(|&(tick, _)| tick as i64)
                .chain((-24000..48000).step_by(37))
                .collect::<Vec<_>>();

            for tick in ticks {
                let sampled = as_channels(&sampler.sample_argument(tick));
                let expected = daylight_track(&table, tick as f32);
                for channel in 0..if is_color { 4 } else { 1 } {
                    let (got, want) = (sampled[channel], expected[channel]);
                    // an integer channel lerp floors, so a byte channel may sit
                    // one below the float oracle
                    let tolerance = if is_color { 1.0 } else { 1e-5 };
                    assert!(
                        (got - want).abs() <= tolerance,
                        "{id} channel {channel} at {tick}: got {got}, daylight.rs says {want}"
                    );
                }
            }
        }
    }

    #[test]
    fn sun_angle_agrees_with_the_daylight_closed_form_across_a_day() {
        let day = timeline("day.json");
        let sun = day.tracks["minecraft:visual/sun_angle"].bake(day.period_ticks);
        let moon = day.tracks["minecraft:visual/moon_angle"].bake(day.period_ticks);

        let mut worst: f32 = 0.0;
        for tick in 0..24000 {
            let sampled = float(&sun.sample_argument(tick));
            worst = worst.max((sampled - daylight_sun_angle(tick as f32)).abs());
            assert_eq!(
                float(&moon.sample_argument(tick)),
                sampled + 180.0,
                "the moon trails the sun by half a revolution at {tick}"
            );
        }
        assert!(
            worst < 0.1,
            "the bezier drifts {worst} degrees from the closed form"
        );

        // the anchors daylight.rs pins: noon starts the revolution, dusk is a
        // quarter of the way round, midnight is halfway
        assert_eq!(float(&sun.sample_argument(6000)), 0.0);
        assert!((float(&sun.sample_argument(18000)) - 180.0).abs() < 0.1);
        assert!(float(&sun.sample_argument(12000)) > 60.0);
        assert!(float(&sun.sample_argument(12000)) < 90.0);
    }

    #[test]
    fn two_keyframes_on_one_tick_make_a_full_revolution() {
        let day = timeline("day.json");
        let sun = day.tracks["minecraft:visual/sun_angle"].bake(day.period_ticks);

        // the zero-length middle segment is what makes tick 6000 read as 0
        assert_eq!(float(&sun.sample_argument(6000)), 0.0);
        assert!(
            float(&sun.sample_argument(5999)) > 359.9,
            "and the tick before it as 360"
        );
        assert!(float(&sun.sample_argument(6001)) < 0.1);
        let mut previous = 0.0;
        for tick in 6001..30000 {
            let angle = float(&sun.sample_argument(tick));
            assert!(angle >= previous, "sun_angle went backwards at {tick}");
            previous = angle;
        }
    }

    #[test]
    fn a_periodic_track_wraps_instead_of_clamping() {
        let day = timeline("day.json");
        let id = "minecraft:visual/sky_light_factor";
        let sampler = day.tracks[id].bake(day.period_ticks);
        let at = |tick| float(&sampler.sample_argument(tick));

        // the keyframes run 730 → 22860, so 100 is before the first and 23000
        // after the last; both must interpolate around the period
        let before = at(100);
        let after = at(23000);
        assert!(
            before > 0.24 && before < 1.0,
            "before the first keyframe: {before}"
        );
        assert!(
            after > 0.24 && after < 1.0,
            "after the last keyframe: {after}"
        );
        assert!(after < before, "dawn climbs back towards full daylight");
        assert_eq!(
            at(-1000),
            at(23000),
            "negative ticks floor-mod into the period"
        );
        assert_eq!(at(24100), before);

        // the plateaus daylight.rs pins
        assert_eq!(at(6000), 1.0);
        assert_eq!(at(18000), 0.24);
    }

    #[test]
    fn a_track_without_a_period_neither_wraps_nor_reduces_its_ticks() {
        let early = timeline("early_game.json");
        assert_eq!(early.period_ticks, None);
        let id = "minecraft:gameplay/can_pillager_patrol_spawn";
        let sampler = early.bake().remove(id).unwrap();
        let at = |tick| sampler.sample_argument(tick);

        let (no, yes) = (AttributeValue::Bool(false), AttributeValue::Bool(true));
        assert_eq!(at(-50_000), no, "before the first keyframe the track holds");
        assert_eq!(at(0), no);
        // boolean arguments step, so the switch happens at the far keyframe
        assert_eq!(at(119_999), no);
        assert_eq!(at(120_000), yes);
        assert_eq!(at(1_000_000), yes, "and never wraps back round");

        assert_eq!(sampler.modifier, Operation::And);
        assert_eq!(sampler.apply(&yes, 0).unwrap(), no);
        assert_eq!(sampler.apply(&yes, 120_000).unwrap(), yes);
    }

    #[test]
    fn a_not_interpolated_track_holds_each_value_for_a_whole_segment() {
        let moon = timeline("moon.json");
        let id = "minecraft:visual/moon_phase";
        let sampler = moon.tracks[id].bake(moon.period_ticks);
        let at = |tick| sampler.sample_argument(tick);
        let phase = AttributeValue::MoonPhase;

        // nominally a linear track, but MOON_PHASE is not interpolated, so each
        // phase holds for a whole day instead of blending into the next
        assert_eq!(at(0), phase(MoonPhase::FullMoon));
        assert_eq!(at(12_000), phase(MoonPhase::FullMoon));
        assert_eq!(at(23_999), phase(MoonPhase::FullMoon));
        assert_eq!(at(24_000), phase(MoonPhase::WaningGibbous));
        assert_eq!(
            at(192_000),
            phase(MoonPhase::FullMoon),
            "the period brings it back round"
        );
        assert_eq!(
            at(-1),
            phase(MoonPhase::WaxingGibbous),
            "and the tick before it is the last phase"
        );
    }

    #[test]
    fn a_constant_easing_holds_until_the_next_keyframe() {
        let day = timeline("day.json");
        let id = "minecraft:gameplay/cat_waking_up_gift_chance";
        let sampler = day.tracks[id].bake(day.period_ticks);
        assert_eq!(sampler.argument.easing(), Easing::Constant);

        // keyframes are 362 → 0.0 and 23667 → 0.7
        assert_eq!(float(&sampler.sample_argument(362)), 0.0);
        assert_eq!(float(&sampler.sample_argument(12_000)), 0.0);
        assert_eq!(float(&sampler.sample_argument(23_666)), 0.0);
        assert_eq!(float(&sampler.sample_argument(23_667)), 0.7);
    }

    #[test]
    fn malformed_tracks_are_rejected_at_load() {
        let track = |keyframes: Vec<(u32, f32)>| {
            json!({
                "keyframes": keyframes
                    .into_iter()
                    .map(|(ticks, value)| json!({"ticks": ticks, "value": value}))
                    .collect::<Vec<_>>(),
                "modifier": "multiply",
            })
        };
        let bake = |keyframes, period| {
            bake_one(
                "minecraft:visual/sky_light_factor",
                period,
                track(keyframes),
            )
        };

        assert!(
            bake(vec![], Some(24000)).is_err(),
            "keyframes must not be empty"
        );
        assert!(
            bake(vec![(100, 1.0), (50, 0.0)], Some(24000)).is_err(),
            "must be ordered"
        );
        assert!(
            bake(vec![(50, 1.0), (50, 0.5), (50, 0.0)], Some(24000)).is_err(),
            "at most two keyframes may share a tick"
        );
        assert!(
            bake(vec![(50, 1.0), (50, 0.0)], Some(24000)).is_ok(),
            "but two may"
        );
        assert!(
            bake(vec![(0, 1.0), (24001, 0.0)], Some(24000)).is_err(),
            "must be within period"
        );
        assert!(
            bake(vec![(0, 1.0), (24001, 0.0)], None).is_ok(),
            "unless there is no period"
        );

        let unknown = load(
            "minecraft:visual/sky_colour",
            None,
            json!({"keyframes": [{"ticks": 0, "value": "#ffffff"}]}),
        )
        .unwrap_err();
        assert!(
            unknown.contains("is not an environment attribute"),
            "{unknown}"
        );

        let bad_modifier = load(
            "minecraft:visual/sky_light_factor",
            None,
            json!({"keyframes": [{"ticks": 0, "value": 1.0}], "modifier": "blend"}),
        )
        .unwrap_err();
        assert!(
            bad_modifier.contains("unknown variant `blend`"),
            "{bad_modifier}"
        );

        let wrong_modifier = load(
            "minecraft:visual/sky_light_factor",
            None,
            json!({"keyframes": [{"ticks": 0, "value": 1.0}], "modifier": "or"}),
        )
        .unwrap_err();
        assert!(
            wrong_modifier.contains("Or is not a valid modifier for Float"),
            "{wrong_modifier}"
        );

        let wrong_value = load(
            "minecraft:visual/sky_light_factor",
            None,
            json!({"keyframes": [{"ticks": 0, "value": "noon"}], "modifier": "multiply"}),
        )
        .unwrap_err();
        assert!(
            wrong_value.contains("is not a valid Float value"),
            "{wrong_value}"
        );
    }

    #[test]
    fn a_single_keyframe_track_is_a_constant() {
        for period in [Some(24000), None] {
            let sampler = bake_one(
                "minecraft:visual/sky_light_factor",
                period,
                json!({
                    "keyframes": [{"ticks": 500, "value": 0.25}],
                    "modifier": "multiply",
                }),
            )
            .unwrap();
            for tick in [-1_000_000, -1, 0, 500, 23_999, 1_000_000] {
                assert_eq!(float(&sampler.sample_argument(tick)), 0.25, "at {tick}");
            }
        }
    }

    fn read_document(doc: Value) -> Result<Timeline, String> {
        TEST_CLOCKS.scope(|| serde_json::from_value(doc).map_err(|e| e.to_string()))
    }

    #[test]
    fn a_marker_on_the_period_boundary_is_rejected() {
        let with = |markers: Value| {
            read_document(json!({
                "clock": "minecraft:overworld",
                "period_ticks": 24_000,
                "time_markers": markers,
            }))
        };

        let refused = with(json!({"minecraft:day": 24_000, "minecraft:noon": 23_999})).unwrap_err();
        assert!(
            refused.contains("`minecraft:day` at tick 24000 must be in range [0; 24000)"),
            "{refused}"
        );
        assert!(with(json!({"minecraft:noon": 23_999})).is_ok());
    }

    #[test]
    fn a_keyframe_on_the_period_boundary_is_still_accepted() {
        let timeline = load(
            "minecraft:visual/star_brightness",
            Some(24_000),
            json!({"keyframes": [{"ticks": 0, "value": 0.0}, {"ticks": 24_000, "value": 1.0}]}),
        )
        .unwrap();
        assert_eq!(timeline.bake().len(), 1);
    }

    #[test]
    fn a_timeline_and_its_markers_refuse_what_the_game_does_not_know() {
        let refusals = [
            (
                json!({"clock": "minecraft:overworld", "extra": 1}),
                "unknown field `extra`",
            ),
            (
                json!({"clock": "minecraft:overworld", "time_markers": {"minecraft:day": {"ticks": 1, "extra": true}}}),
                "unknown field `extra`",
            ),
            (
                json!({"clock": "minecraft:overworld", "time_markers": {"minecraft:day": -1}}),
                "invalid type: integer `-1`",
            ),
            (
                json!({"clock": "minecraft:overworld", "time_markers": {"Not A Name": 1}}),
                "Not A Name",
            ),
            (json!({"clock": "minecraft:nowhere"}), "minecraft:nowhere"),
        ];
        for (doc, expected) in refusals {
            let refused = read_document(doc.clone()).unwrap_err();
            assert!(refused.contains(expected), "{doc}: {refused}");
        }
    }
}
