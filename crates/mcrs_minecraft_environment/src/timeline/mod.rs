use mcrs_minecraft_resource::ResourceKey;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::world_clock::WorldClock;
use mcrs_minecraft_core::codec::{NonNegativeInt, PositiveInt};
use mcrs_minecraft_registries::{Named, Refs, Resolve, Resolved};
use mcrs_minecraft_resource::ResourceLocation;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

mod marker;
mod sampler;
mod track;

pub use marker::TimeMarker;
pub use sampler::{AttributeTrackSampler, TrackSampler};
pub use track::{Easing, Keyframe, Track, TrackError, Tracks};

pub type TimeMarkers = BTreeMap<ResourceLocation<Arc<str>>, TimeMarker>;

fn non_negative_ticks<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    NonNegativeInt::deserialize(d).map(|ticks| ticks.0 as u32)
}

#[derive(Debug, Clone, Resolve)]
pub struct Timeline<M: Refs = Resolved> {
    pub clock: M::Id<WorldClock>,
    pub period_ticks: Option<u32>,
    pub tracks: Tracks,
    pub time_markers: TimeMarkers,
}

impl Serialize for Timeline<Named> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut fields = serializer.serialize_struct("Timeline", 4)?;
        fields.serialize_field("clock", &self.clock)?;
        if let Some(period) = self.period_ticks {
            fields.serialize_field("period_ticks", &period)?;
        }
        if !self.tracks.is_empty() {
            fields.serialize_field("tracks", &self.tracks)?;
        }
        if !self.time_markers.is_empty() {
            fields.serialize_field("time_markers", &self.time_markers)?;
        }
        fields.end()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TimelineRepr {
    clock: ResourceKey<WorldClock>,
    #[serde(default)]
    period_ticks: Option<PositiveInt>,
    #[serde(default)]
    tracks: Tracks,
    #[serde(default)]
    time_markers: TimeMarkers,
}

impl<'de> Deserialize<'de> for Timeline<Named> {
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
#[derive(Debug, Clone, Resolve)]
pub struct NetworkTimeline<M: Refs = Resolved>(Timeline<M>);

impl Serialize for NetworkTimeline<Named> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NetworkTimeline<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let timeline = Timeline::<Named>::deserialize(d)?;
        Ok(NetworkTimeline(Timeline {
            tracks: timeline.tracks.syncable(),
            ..timeline
        }))
    }
}

impl From<NetworkTimeline> for Timeline {
    fn from(timeline: NetworkTimeline) -> Self {
        timeline.0
    }
}

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
    use crate::attribute::{AttributeValue, MoonPhase};
    use crate::world_clock::TEST_CLOCKS;
    use mcrs_minecraft_nbt::tag::NbtTag;
    use mcrs_minecraft_worldgen_testing::{reencode, vanilla_dir};
    use serde_json::{Value, json};

    const SHIPPED: [&str; 4] = [
        "day.json",
        "moon.json",
        "villager_schedule.json",
        "early_game.json",
    ];

    fn raw(name: &str) -> Value {
        let bytes = std::fs::read(vanilla_dir().join("timeline").join(name)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn timeline(name: &str) -> Timeline {
        TEST_CLOCKS.read(raw(name)).unwrap()
    }

    /// A one-track timeline read the way a datapack reaches one.
    fn load(attribute: &str, period_ticks: Option<u32>, track: Value) -> Result<Timeline, String> {
        let mut doc = json!({"clock": "minecraft:overworld", "tracks": {attribute: track}});
        if let Some(period) = period_ticks {
            doc["period_ticks"] = json!(period);
        }
        TEST_CLOCKS.read(doc)
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
        reencode(&TEST_CLOCKS.named(timeline))
    }

    fn sent(timeline: &Timeline) -> Value {
        reencode(&TEST_CLOCKS.named(&NetworkTimeline::from(timeline)))
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
            let read = TEST_CLOCKS.read::<Timeline>(document.clone());
            assert!(read.is_err(), "{field}: {document} loads");
        }
        let edge = TEST_CLOCKS.read::<Timeline>(json!({
            "clock": "minecraft:overworld",
            "time_markers": {"minecraft:noon": i32::MAX},
            "tracks": track(json!(i32::MAX)),
        }));
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
            Some(Easing::CubicBezier([0.362, 0.241, 0.638, 0.759]))
        );
        assert_eq!(
            day.tracks["minecraft:gameplay/cat_waking_up_gift_chance"].ease,
            Some(Easing::Named("constant".to_owned()))
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
        let nbt = mcrs_minecraft_nbt::to_nbt_compound(&TEST_CLOCKS.named(&day)).unwrap();

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
        let moon = mcrs_minecraft_nbt::to_nbt_compound(&TEST_CLOCKS.named(&timeline("moon.json")))
            .unwrap();
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

    fn float(value: &AttributeValue) -> f32 {
        match value {
            AttributeValue::Float(v) => *v,
            other => panic!("expected a float, got {other:?}"),
        }
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
                    let sampled = sampler.sample_argument(ticks).unwrap();
                    assert_eq!(
                        sampled,
                        sampler.sample_argument(ticks).unwrap(),
                        "{name} / {id} at {ticks} is not a pure function of ticks"
                    );
                }
            }
        }
    }

    #[test]
    fn two_keyframes_on_one_tick_make_a_full_revolution() {
        let day = timeline("day.json");
        let sun = day.tracks["minecraft:visual/sun_angle"].bake(day.period_ticks);

        // the zero-length middle segment is what makes tick 6000 read as 0
        assert_eq!(float(&sun.sample_argument(6000).unwrap()), 0.0);
        assert!(
            float(&sun.sample_argument(5999).unwrap()) > 359.9,
            "and the tick before it as 360"
        );
        assert!(float(&sun.sample_argument(6001).unwrap()) < 0.1);
        let mut previous = 0.0;
        for tick in 6001..30000 {
            let angle = float(&sun.sample_argument(tick).unwrap());
            assert!(angle >= previous, "sun_angle went backwards at {tick}");
            previous = angle;
        }
    }

    #[test]
    fn a_periodic_track_wraps_instead_of_clamping() {
        let day = timeline("day.json");
        let id = "minecraft:visual/sky_light_factor";
        let sampler = day.tracks[id].bake(day.period_ticks);
        let at = |tick| float(&sampler.sample_argument(tick).unwrap());

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
    fn a_not_interpolated_track_holds_each_value_for_a_whole_segment() {
        let moon = timeline("moon.json");
        let id = "minecraft:visual/moon_phase";
        let sampler = moon.tracks[id].bake(moon.period_ticks);
        let at = |tick| sampler.sample_argument(tick).unwrap();
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
                assert_eq!(
                    float(&sampler.sample_argument(tick).unwrap()),
                    0.25,
                    "at {tick}"
                );
            }
        }
    }

    fn read_document(doc: Value) -> Result<Timeline, String> {
        TEST_CLOCKS.read(doc)
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
