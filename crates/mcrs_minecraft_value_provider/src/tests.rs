use crate::{
    BoundedIntProvider, DispatchedFloatProvider, DispatchedHeightProvider, DispatchedIntProvider,
    FloatProvider, HeightContext, HeightProvider, IntProvider, VerticalAnchor, Weighted,
};
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use serde::{Deserialize, Serialize};

const OVERWORLD: HeightContext = HeightContext {
    min_y: -64,
    depth: 384,
    sea_level: 63,
};

fn round_trip<T>(json: &str) -> T
where
    T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let parsed: T = serde_json::from_str(json).expect("parses");
    let written = serde_json::to_string(&parsed).expect("writes");
    let again: T = serde_json::from_str(&written).expect("reparses");
    assert_eq!(parsed, again, "round trip changed the value: {written}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(json).unwrap(),
        serde_json::from_str::<serde_json::Value>(&written).unwrap(),
        "round trip changed the json"
    );
    parsed
}

/// Runs a sampler and an independently written replay of its reference draw
/// sequence from the same seed. Equal values prove the arithmetic, equal RNG
/// state proves the number and order of draws.
#[track_caller]
fn pin_draws<T: PartialEq + std::fmt::Debug>(
    sample: impl FnOnce(&mut XoroshiroRandom) -> T,
    replay: impl FnOnce(&mut XoroshiroRandom) -> T,
) -> T {
    let mut sampled = XoroshiroRandom::new(0x5eed);
    let mut reference = XoroshiroRandom::new(0x5eed);
    let value = sample(&mut sampled);
    assert_eq!(value, replay(&mut reference), "sampled value");
    assert_eq!(sampled, reference, "draw sequence");
    value
}

#[test]
fn a_bare_number_is_a_constant() {
    assert_eq!(round_trip::<IntProvider>("7"), IntProvider::Constant(7));
    assert_eq!(
        round_trip::<FloatProvider>("-0.7"),
        FloatProvider::Constant(-0.7)
    );
}

/// The corpus never writes it, but `constant` is a member of all three provider
/// registries, so a datapack may. The reference encodes a constant through
/// `Either.left(value)`, so the object form is accepted and written back bare.
#[test]
fn the_explicit_constant_form_loads_and_normalises() {
    assert_eq!(
        serde_json::from_str::<IntProvider>(r#"{"type":"minecraft:constant","value":7}"#)
            .expect("parses"),
        IntProvider::Constant(7)
    );
    assert_eq!(
        serde_json::from_str::<FloatProvider>(r#"{"type":"minecraft:constant","value":-0.7}"#)
            .expect("parses"),
        FloatProvider::Constant(-0.7)
    );
    assert_eq!(
        serde_json::from_str::<HeightProvider>(
            r#"{"type":"minecraft:constant","value":{"below_top":1}}"#
        )
        .expect("parses"),
        HeightProvider::Constant(VerticalAnchor::BelowTop(1))
    );

    assert_eq!(
        serde_json::to_string(&IntProvider::Constant(7)).expect("writes"),
        "7"
    );
    assert_eq!(
        serde_json::to_string(&FloatProvider::Constant(-0.7)).expect("writes"),
        "-0.7"
    );
    assert_eq!(
        serde_json::to_string(&HeightProvider::Constant(VerticalAnchor::BelowTop(1)))
            .expect("writes"),
        r#"{"below_top":1}"#
    );

    assert!(
        serde_json::from_str::<IntProvider>(r#"{"type":"minecraft:constant","value":7,"x":1}"#)
            .is_err(),
        "an unknown sibling field is a load error"
    );
}

#[test]
fn the_carver_registry_forms_round_trip() {
    assert_eq!(
        round_trip::<IntProvider>(
            r#"{"type":"minecraft:very_biased_to_bottom","min_inclusive":0,"max_inclusive":14}"#
        ),
        IntProvider::Dispatched(DispatchedIntProvider::VeryBiasedToBottom {
            min_inclusive: 0,
            max_inclusive: 14
        })
    );
    assert_eq!(
        round_trip::<FloatProvider>(
            r#"{"type":"minecraft:trapezoid","min":0.0,"max":3.0,"plateau":1.0}"#
        ),
        FloatProvider::Dispatched(DispatchedFloatProvider::Trapezoid {
            min: 0.0,
            max: 3.0,
            plateau: 1.0
        })
    );
    assert_eq!(
        round_trip::<FloatProvider>(
            r#"{"type":"minecraft:uniform","min_inclusive":0.7,"max_exclusive":1.4}"#
        ),
        FloatProvider::Dispatched(DispatchedFloatProvider::Uniform {
            min_inclusive: 0.7,
            max_exclusive: 1.4
        })
    );
    assert_eq!(
        round_trip::<HeightProvider>(
            r#"{"type":"minecraft:uniform","min_inclusive":{"above_bottom":8},"max_inclusive":{"absolute":180}}"#
        ),
        HeightProvider::Dispatched(DispatchedHeightProvider::Uniform {
            min_inclusive: VerticalAnchor::AboveBottom(8),
            max_inclusive: VerticalAnchor::Absolute(180)
        })
    );
    assert_eq!(
        round_trip::<HeightProvider>(r#"{"below_top":1}"#),
        HeightProvider::Constant(VerticalAnchor::BelowTop(1))
    );
    assert_eq!(
        round_trip::<HeightProvider>(r#"{"relative_to_sea_level":0}"#),
        HeightProvider::Constant(VerticalAnchor::RelativeToSeaLevel(0))
    );
}

/// Every shape the feature and placed_feature corpus carries, verbatim.
#[test]
fn the_feature_registry_forms_round_trip() {
    round_trip::<IntProvider>(r#"{"type":"minecraft:trapezoid","max":7,"min":-7,"plateau":0}"#);
    round_trip::<IntProvider>(
        r#"{"type":"minecraft:biased_to_bottom","max_inclusive":2,"min_inclusive":1}"#,
    );
    round_trip::<IntProvider>(
        r#"{"type":"minecraft:clamped","max_inclusive":1,"min_inclusive":0,"source":{"type":"minecraft:uniform","max_inclusive":1,"min_inclusive":-3}}"#,
    );
    round_trip::<IntProvider>(
        r#"{"type":"minecraft:clamped_normal","deviation":3.0,"max_inclusive":10,"mean":0.0,"min_inclusive":-10}"#,
    );
    round_trip::<IntProvider>(
        r#"{"type":"minecraft:weighted_list","distribution":[{"data":0,"weight":3},{"data":{"type":"minecraft:uniform","max_inclusive":4,"min_inclusive":1},"weight":1}]}"#,
    );
    round_trip::<FloatProvider>(
        r#"{"type":"minecraft:clamped_normal","deviation":0.3,"max":0.9,"mean":0.1,"min":0.1}"#,
    );
    round_trip::<HeightProvider>(
        r#"{"type":"minecraft:trapezoid","max_inclusive":{"absolute":192},"min_inclusive":{"absolute":0}}"#,
    );
    round_trip::<HeightProvider>(
        r#"{"type":"minecraft:very_biased_to_bottom","inner":8,"max_inclusive":{"below_top":8},"min_inclusive":{"above_bottom":0}}"#,
    );
}

#[test]
fn a_vertical_anchor_with_two_keys_or_none_is_a_load_error() {
    for anchor in [
        r#"{"absolute":1,"above_bottom":2}"#,
        "{}",
        r#"{"below_surface":1}"#,
    ] {
        assert!(
            serde_json::from_str::<VerticalAnchor>(anchor).is_err(),
            "{anchor} as text"
        );
        let tagged = format!(
            r#"{{"type":"minecraft:uniform","min_inclusive":{anchor},"max_inclusive":{{"absolute":0}}}}"#
        );
        assert!(
            serde_json::from_str::<HeightProvider>(&tagged).is_err(),
            "{anchor} inside a tagged height provider"
        );
        assert!(
            serde_json::from_str::<HeightProvider>(anchor).is_err(),
            "{anchor} as a bare height provider"
        );
    }
}

#[test]
fn a_vertical_anchor_offset_is_bounded_as_the_reference_bounds_it() {
    for key in [
        "absolute",
        "above_bottom",
        "below_top",
        "relative_to_sea_level",
    ] {
        for offset in [-2032, 2031] {
            let json = format!(r#"{{"{key}":{offset}}}"#);
            round_trip::<VerticalAnchor>(&json);
            round_trip::<HeightProvider>(&json);
        }
        for offset in [-2033, 2032] {
            let json = format!(r#"{{"{key}":{offset}}}"#);
            assert!(
                serde_json::from_str::<VerticalAnchor>(&json).is_err(),
                "{json} as text"
            );
            assert!(
                serde_json::from_str::<HeightProvider>(&json).is_err(),
                "{json} as a bare height provider"
            );
        }
    }
}

/// `plateau` and `inner` are `optionalFieldOf` in the reference, so their
/// defaults must not be written back out.
#[test]
fn the_height_defaults_are_omitted_on_the_way_out() {
    let trapezoid = HeightProvider::Dispatched(DispatchedHeightProvider::Trapezoid {
        min_inclusive: VerticalAnchor::Absolute(0),
        max_inclusive: VerticalAnchor::Absolute(16),
        plateau: 0,
    });
    assert!(
        !serde_json::to_string(&trapezoid)
            .unwrap()
            .contains("plateau")
    );
    let biased = HeightProvider::Dispatched(DispatchedHeightProvider::VeryBiasedToBottom {
        min_inclusive: VerticalAnchor::Absolute(0),
        max_inclusive: VerticalAnchor::Absolute(16),
        inner: Bounded(1),
    });
    assert!(!serde_json::to_string(&biased).unwrap().contains("inner"));
}

#[test]
fn an_unnamed_form_is_a_load_error() {
    assert!(serde_json::from_str::<IntProvider>(r#"{"type":"minecraft:not_a_provider"}"#).is_err());
    assert!(serde_json::from_str::<IntProvider>(r#"{"type":"minecraft:clamped"}"#).is_err());
    assert!(
        serde_json::from_str::<HeightProvider>(
            r#"{"type":"minecraft:weighted_list","distribution":[]}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<IntProvider>(
            r#"{"type":"minecraft:weighted_list","distribution":[]}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<IntProvider>(
            r#"{"type":"minecraft:weighted_list","distribution":[{"data":1,"weight":0}]}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<FloatProvider>(
            r#"{"type":"minecraft:uniform","min_inclusive":0.0,"max_exclusive":1.0,"extra":1}"#
        )
        .is_err()
    );
}

#[test]
fn a_malformed_typed_provider_reports_its_own_error() {
    fn error<T: for<'de> Deserialize<'de> + std::fmt::Debug>(json: &str) -> String {
        serde_json::from_str::<T>(json).unwrap_err().to_string()
    }
    for (json, message) in [
        (
            error::<IntProvider>(
                r#"{"type":"minecraft:very_biased_to_bottom","min_inclusive":5,"max_inclusive":2}"#,
            ),
            "Max must be at least min: [5, 2]",
        ),
        (
            error::<IntProvider>(r#"{"type":"minecraft:uniform","min_inclusive":0}"#),
            "missing field `max_inclusive`",
        ),
        (
            error::<FloatProvider>(r#"{"type":"minecraft:uniform","min_inclusive":0.0}"#),
            "missing field `max_exclusive`",
        ),
        (
            error::<HeightProvider>(
                r#"{"type":"minecraft:uniform","min_inclusive":{"absolute":0}}"#,
            ),
            "missing field `max_inclusive`",
        ),
    ] {
        assert!(json.contains(message), "{json:?} lacks {message:?}");
    }
}

#[test]
fn an_anchor_resolves_against_the_dimension_extent() {
    assert_eq!(VerticalAnchor::Absolute(180).resolve_y(OVERWORLD), 180);
    assert_eq!(VerticalAnchor::AboveBottom(8).resolve_y(OVERWORLD), -56);
    assert_eq!(VerticalAnchor::BelowTop(1).resolve_y(OVERWORLD), 318);
    assert_eq!(
        VerticalAnchor::RelativeToSeaLevel(3).resolve_y(OVERWORLD),
        66
    );
}

/// Every provider draws exactly the values Java's own `sample` does, in the
/// same order, so a shared seed has to produce the same numbers.
#[test]
fn sampling_matches_the_reference_draw_order() {
    let mut rng = LegacyRandom::new(42);
    let mut reference = LegacyRandom::new(42);

    let uniform_int = IntProvider::Dispatched(DispatchedIntProvider::Uniform {
        min_inclusive: 3,
        max_inclusive: 9,
    });
    assert_eq!(
        uniform_int.sample(&mut rng),
        reference.next_i32_bound(9 - 3 + 1) + 3
    );

    let very_biased = IntProvider::Dispatched(DispatchedIntProvider::VeryBiasedToBottom {
        min_inclusive: 0,
        max_inclusive: 14,
    });
    let expected = {
        let a = reference.next_i32_bound(15) + 1;
        let b = reference.next_i32_bound(a) + 1;
        reference.next_i32_bound(b)
    };
    assert_eq!(very_biased.sample(&mut rng), expected);

    let trapezoid = FloatProvider::Dispatched(DispatchedFloatProvider::Trapezoid {
        min: 0.0,
        max: 3.0,
        plateau: 1.0,
    });
    let expected = {
        let range = 3.0f32;
        let ramp = (range - 1.0) / 2.0;
        reference.next_f32() * (range - ramp) + reference.next_f32() * ramp
    };
    assert_eq!(trapezoid.sample(&mut rng), expected);
}

#[test]
fn biased_to_bottom_spends_two_draws() {
    let provider = IntProvider::Dispatched(DispatchedIntProvider::BiasedToBottom {
        min_inclusive: 1,
        max_inclusive: 2,
    });
    pin_draws(
        |rng| provider.sample(rng),
        |rng| {
            let span = rng.next_i32_bound(2) + 1;
            1 + rng.next_i32_bound(span)
        },
    );
}

#[test]
fn clamped_spends_only_its_sources_draws() {
    let provider = IntProvider::Dispatched(DispatchedIntProvider::Clamped {
        source: Box::new(IntProvider::Dispatched(DispatchedIntProvider::Uniform {
            min_inclusive: -3,
            max_inclusive: 1,
        })),
        min_inclusive: 0,
        max_inclusive: 1,
    });
    pin_draws(
        |rng| provider.sample(rng),
        |rng| (rng.next_i32_bound(5) - 3).clamp(0, 1),
    );
}

#[test]
fn a_weighted_list_draws_the_selection_then_the_entry() {
    let provider = IntProvider::Dispatched(DispatchedIntProvider::WeightedList {
        distribution: vec![
            Weighted {
                data: IntProvider::Constant(5),
                weight: Bounded(3),
            },
            Weighted {
                data: IntProvider::Dispatched(DispatchedIntProvider::Uniform {
                    min_inclusive: 10,
                    max_inclusive: 12,
                }),
                weight: Bounded(1),
            },
        ]
        .try_into()
        .unwrap(),
    });
    pin_draws(
        |rng| provider.sample(rng),
        |rng| {
            if rng.next_i32_bound(4) < 3 {
                5
            } else {
                rng.next_i32_bound(3) + 10
            }
        },
    );
}

/// `IntProviders.codec(0, 16)` judges a provider by its whole range, so a
/// uniform that reaches past the bound is refused even though every value
/// it could draw might be in range on a given day.
#[test]
fn a_bounded_provider_is_judged_by_its_range() {
    type Size = BoundedIntProvider<0, 16>;
    assert_eq!(
        serde_json::from_str::<Size>("7").unwrap().0,
        IntProvider::Constant(7)
    );
    let uniform = r#"{"type":"minecraft:uniform","min_inclusive":2,"max_inclusive":16}"#;
    assert_eq!(
        serde_json::from_str::<Size>(uniform).unwrap().0,
        IntProvider::uniform(2, 16)
    );
    let too_high = r#"{"type":"minecraft:uniform","min_inclusive":2,"max_inclusive":17}"#;
    let error = serde_json::from_str::<Size>(too_high)
        .unwrap_err()
        .to_string();
    assert!(
        error.starts_with("Value provider too high: 16 [2-17]"),
        "{error}"
    );
    let error = serde_json::from_str::<Size>("-1").unwrap_err().to_string();
    assert!(
        error.starts_with("Value provider too low: 0 [-1--1]"),
        "{error}"
    );
}

#[test]
fn clamped_normal_spends_one_gaussian() {
    let int = IntProvider::Dispatched(DispatchedIntProvider::ClampedNormal {
        mean: 0.0,
        deviation: 3.0,
        min_inclusive: -10,
        max_inclusive: 10,
    });
    pin_draws(
        |rng| int.sample(rng),
        |rng| (rng.next_gaussian() as f32 * 3.0).clamp(-10.0, 10.0) as i32,
    );

    let float = FloatProvider::Dispatched(DispatchedFloatProvider::ClampedNormal {
        mean: 0.1,
        deviation: 0.3,
        min: 0.1,
        max: 0.9,
    });
    pin_draws(
        |rng| float.sample(rng),
        |rng| (0.1 + rng.next_gaussian() as f32 * 0.3).clamp(0.1, 0.9),
    );
}

/// The symmetric zero-plateau form the `offset` modifier uses is a difference
/// of two draws, not the two-ramp sum.
#[test]
fn the_symmetric_int_trapezoid_is_a_difference_of_two_draws() {
    let provider = IntProvider::Dispatched(DispatchedIntProvider::Trapezoid {
        min: -7,
        max: 7,
        plateau: 0,
    });
    let value = pin_draws(
        |rng| provider.sample(rng),
        |rng| rng.next_i32_bound(8) - rng.next_i32_bound(8),
    );
    assert!((-7..=7).contains(&value));
}

#[test]
fn an_asymmetric_int_trapezoid_sums_two_ramps() {
    let provider = IntProvider::Dispatched(DispatchedIntProvider::Trapezoid {
        min: 2,
        max: 11,
        plateau: 3,
    });
    pin_draws(
        |rng| provider.sample(rng),
        |rng| 2 + rng.next_i32_bound(7) + rng.next_i32_bound(4),
    );
}

#[test]
fn a_full_plateau_int_trapezoid_is_one_uniform_draw() {
    let provider = IntProvider::Dispatched(DispatchedIntProvider::Trapezoid {
        min: 2,
        max: 11,
        plateau: 9,
    });
    pin_draws(|rng| provider.sample(rng), |rng| rng.next_i32_bound(10) + 2);
}

#[test]
fn the_height_trapezoid_sums_two_ramps() {
    let provider = HeightProvider::Dispatched(DispatchedHeightProvider::Trapezoid {
        min_inclusive: VerticalAnchor::Absolute(0),
        max_inclusive: VerticalAnchor::Absolute(192),
        plateau: 0,
    });
    pin_draws(
        |rng| provider.sample(rng, OVERWORLD),
        |rng| rng.next_i32_bound(97) + rng.next_i32_bound(97),
    );
}

/// `VeryBiasedToBottomHeight` is not the int provider of the same name: three
/// guarded draws over shrinking ranges, offset by `inner`.
#[test]
fn the_height_very_biased_to_bottom_spends_three_guarded_draws() {
    let provider = HeightProvider::Dispatched(DispatchedHeightProvider::VeryBiasedToBottom {
        min_inclusive: VerticalAnchor::AboveBottom(0),
        max_inclusive: VerticalAnchor::BelowTop(8),
        inner: Bounded(8),
    });
    let value = pin_draws(
        |rng| provider.sample(rng, OVERWORLD),
        |rng| {
            let (min, max) = (-64, 311);
            let upper = rng.next_i32_bound(max - (min + 8) + 1) + min + 8;
            let biased_upper = rng.next_i32_bound(upper - 1 - min + 1) + min;
            let top = biased_upper - 1 + 8;
            if min >= top {
                min
            } else {
                rng.next_i32_bound(top - min + 1) + min
            }
        },
    );
    assert!((-64..=311).contains(&value), "{value} out of range");
}

#[test]
fn an_empty_height_range_returns_the_floor_without_drawing() {
    let provider = HeightProvider::Dispatched(DispatchedHeightProvider::VeryBiasedToBottom {
        min_inclusive: VerticalAnchor::Absolute(10),
        max_inclusive: VerticalAnchor::Absolute(12),
        inner: Bounded(8),
    });
    pin_draws(|rng| provider.sample(rng, OVERWORLD), |_| 10);
}

#[test]
fn bounds_walk_a_clamped_source() {
    let provider = IntProvider::Dispatched(DispatchedIntProvider::Clamped {
        source: Box::new(IntProvider::Dispatched(DispatchedIntProvider::Uniform {
            min_inclusive: 3,
            max_inclusive: 19,
        })),
        min_inclusive: 3,
        max_inclusive: 16,
    });
    assert_eq!(provider.bounds(), (3, 16));
}
