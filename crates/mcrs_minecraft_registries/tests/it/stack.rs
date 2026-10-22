use std::collections::BTreeSet;
use std::fs;

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_registries::{
    AvailablePack, Diagnostic, Features, KnownPack, Level, PackSelection, pack_stack,
};

use mcrs_minecraft_fixtures::vanilla_dir;

use crate::support::{META, rl};

fn features(world: Option<&[&str]>) -> Features {
    Features {
        known: vec![rl("vanilla"), rl("trade_rebalance"), rl("t:extra")],
        vanilla: vec![rl("vanilla")],
        world: world.map(|w| w.iter().map(|f| rl(f)).collect()),
    }
}

fn selection(enabled: &[&str], disabled: &[&str]) -> PackSelection {
    PackSelection {
        enabled: enabled.iter().map(|s| s.to_string()).collect(),
        disabled: disabled.iter().map(|s| s.to_string()).collect(),
    }
}

const EXTRA: &str = r#"{"pack":{"description":"x","min_format":122,"max_format":122},"features":{"enabled":["t:extra"]}}"#;

struct Case {
    name: &'static str,
    selection: PackSelection,
    available: Vec<AvailablePack>,
    world_features: Option<&'static [&'static str]>,
    safe_mode: bool,
    stack: &'static [&'static str],
    diagnostics: Vec<(Diagnostic, Level)>,
}

fn assert_stacks(cases: Vec<Case>) {
    for case in cases {
        let (stack, diagnostics) = pack_stack(
            &case.selection,
            &case.available,
            &features(case.world_features),
            case.safe_mode,
        );
        let ids: Vec<&str> = stack.packs().iter().map(|p| p.id()).collect();
        assert_eq!(ids, case.stack, "{}", case.name);
        let found: Vec<(Diagnostic, Level)> =
            diagnostics.iter().map(|d| (d.clone(), d.level())).collect();
        assert_eq!(found, case.diagnostics, "{}", case.name);
    }
}

#[test]
fn stack_is_built_from_the_selection_then_new_packs_in_id_order() {
    assert_stacks(vec![
        Case {
            name: "a new directory is appended above the selection",
            selection: selection(&["vanilla", "file/b"], &[]),
            available: vec![
                AvailablePack::vanilla(),
                AvailablePack::directory("b", META),
                AvailablePack::directory("a", META),
            ],
            world_features: Some(&["vanilla"]),
            safe_mode: false,
            stack: &["vanilla", "file/b", "file/a"],
            diagnostics: vec![(
                Diagnostic::PackFound {
                    pack: "file/a".into(),
                },
                Level::Info,
            )],
        },
        Case {
            name: "new packs append in ascending id with the zip name kept whole",
            selection: selection(&["vanilla"], &[]),
            available: vec![
                AvailablePack::vanilla(),
                AvailablePack::directory("b", META),
                AvailablePack::zip("a.zip", Some(META.into())),
            ],
            world_features: None,
            safe_mode: false,
            stack: &["vanilla", "file/a.zip", "file/b"],
            diagnostics: vec![
                (
                    Diagnostic::PackFound {
                        pack: "file/a.zip".into(),
                    },
                    Level::Info,
                ),
                (
                    Diagnostic::PackFound {
                        pack: "file/b".into(),
                    },
                    Level::Info,
                ),
            ],
        },
        Case {
            name: "an unavailable selected pack is dropped as missing",
            selection: selection(&["vanilla", "file/gone"], &[]),
            available: vec![AvailablePack::vanilla()],
            world_features: None,
            safe_mode: false,
            stack: &["vanilla"],
            diagnostics: vec![(
                Diagnostic::MissingPack {
                    pack: "file/gone".into(),
                },
                Level::Warning,
            )],
        },
        Case {
            name: "safe mode keeps only the built-in pack",
            selection: selection(&["vanilla", "file/a"], &[]),
            available: vec![
                AvailablePack::vanilla(),
                AvailablePack::directory("a", META),
            ],
            world_features: None,
            safe_mode: true,
            stack: &["vanilla"],
            diagnostics: vec![],
        },
        Case {
            name: "an empty stack falls back to the built-in pack",
            selection: selection(&[], &["vanilla", "file/a"]),
            available: vec![
                AvailablePack::vanilla(),
                AvailablePack::directory("a", META),
            ],
            world_features: None,
            safe_mode: false,
            stack: &["vanilla"],
            diagnostics: vec![(Diagnostic::EmptyStack, Level::Info)],
        },
    ]);
}

#[test]
fn world_features_gate_which_packs_join_the_stack() {
    assert_stacks(vec![
        Case {
            name: "packs requesting a feature the world lacks are removed or not appended",
            selection: selection(&["vanilla", "file/sel"], &[]),
            available: vec![
                AvailablePack::vanilla(),
                AvailablePack::directory("sel", EXTRA),
                AvailablePack::directory("new", EXTRA),
            ],
            world_features: Some(&["vanilla"]),
            safe_mode: false,
            stack: &["vanilla"],
            diagnostics: vec![
                (
                    Diagnostic::PackFeaturesDisabled {
                        pack: "file/sel".into(),
                        features: vec![rl("t:extra")],
                    },
                    Level::Warning,
                ),
                (
                    Diagnostic::PackNotAppended {
                        pack: "file/new".into(),
                        features: vec![rl("t:extra")],
                    },
                    Level::Info,
                ),
            ],
        },
        Case {
            name: "a pack listed as disabled skips the feature check",
            selection: selection(&["vanilla", "file/sel"], &["file/sel"]),
            available: vec![
                AvailablePack::vanilla(),
                AvailablePack::directory("sel", EXTRA),
            ],
            world_features: Some(&["vanilla"]),
            safe_mode: false,
            stack: &["vanilla", "file/sel"],
            diagnostics: vec![],
        },
    ]);
}

#[test]
fn feature_packs_join_only_for_a_world_feature_no_pack_provides() {
    let trade = fs::read(vanilla_dir().join("datapacks/trade_rebalance/pack.mcmeta")).unwrap();
    assert_stacks(vec![
        Case {
            name: "a world feature no selected pack provides appends its feature pack",
            selection: selection(&["vanilla"], &[]),
            available: vec![
                AvailablePack::vanilla(),
                AvailablePack::feature("trade_rebalance", trade.clone()),
                AvailablePack::feature("t_extra", EXTRA),
            ],
            world_features: Some(&["vanilla", "trade_rebalance"]),
            safe_mode: false,
            stack: &["vanilla", "trade_rebalance"],
            diagnostics: vec![(
                Diagnostic::FeaturePackAppended {
                    pack: "trade_rebalance".into(),
                },
                Level::Info,
            )],
        },
        Case {
            name: "a selected feature pack is not appended twice",
            selection: selection(&["vanilla", "trade_rebalance"], &[]),
            available: vec![
                AvailablePack::vanilla(),
                AvailablePack::feature("trade_rebalance", trade.clone()),
                AvailablePack::feature("t_extra", EXTRA),
            ],
            world_features: Some(&["vanilla", "trade_rebalance", "t:extra"]),
            safe_mode: false,
            stack: &["vanilla", "trade_rebalance", "t_extra"],
            diagnostics: vec![(
                Diagnostic::FeaturePackAppended {
                    pack: "t_extra".into(),
                },
                Level::Info,
            )],
        },
        Case {
            name: "feature packs are not appended while every world feature is provided",
            selection: selection(&["vanilla", "file/sel"], &[]),
            available: vec![
                AvailablePack::vanilla(),
                AvailablePack::directory("sel", EXTRA),
                AvailablePack::feature("t_extra", EXTRA),
            ],
            world_features: Some(&["vanilla", "t:extra"]),
            safe_mode: false,
            stack: &["vanilla", "file/sel"],
            diagnostics: vec![],
        },
        Case {
            name: "a feature pack is never appended as a new pack",
            selection: selection(&["vanilla"], &[]),
            available: vec![
                AvailablePack::vanilla(),
                AvailablePack::feature("trade_rebalance", trade.clone()),
            ],
            world_features: None,
            safe_mode: false,
            stack: &["vanilla"],
            diagnostics: vec![],
        },
    ]);
}

#[test]
fn enabled_features_are_the_union_of_the_stack_and_the_world() {
    let cases: [(Option<&[&str]>, &[&str]); 2] = [
        (None, &["t:extra", "vanilla"]),
        (
            Some(&["vanilla", "t:extra", "trade_rebalance"]),
            &["t:extra", "trade_rebalance", "vanilla"],
        ),
    ];
    for (world, expected) in cases {
        let (stack, _) = pack_stack(
            &selection(&["vanilla", "file/a"], &[]),
            &[
                AvailablePack::vanilla(),
                AvailablePack::directory("a", EXTRA),
            ],
            &features(world),
            false,
        );
        assert_eq!(stack.packs()[1].features(), [rl("t:extra")]);
        let enabled: BTreeSet<_> = stack.enabled_features().iter().cloned().collect();
        let expected: BTreeSet<_> = expected.iter().map(|f| rl(f)).collect();
        assert_eq!(enabled, expected, "{world:?}");
    }
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Available,
    AvailableWithWarning,
    OverlaySkipped,
    Unavailable,
}

fn assert_metadata_outcomes(cases: &[(&str, Option<&str>, Outcome)]) {
    for &(name, meta, ref outcome) in cases {
        let pack = AvailablePack::zip("p.zip", meta.map(|m| m.as_bytes().to_vec()));
        let (stack, diagnostics) = pack_stack(
            &selection(&["vanilla", "file/p.zip"], &[]),
            &[AvailablePack::vanilla(), pack],
            &features(None),
            false,
        );
        let available = stack.packs().iter().any(|pack| pack.id() == "file/p.zip");
        let about_pack: Vec<&Diagnostic> = diagnostics
            .iter()
            .filter(|d| {
                matches!(d,
                    Diagnostic::UnknownCompatibility { pack, .. }
                    | Diagnostic::UnavailablePack { pack, .. }
                    | Diagnostic::OverlaySkipped { pack, .. } if pack == "file/p.zip")
            })
            .collect();
        let observed = match (available, about_pack.as_slice()) {
            (true, []) => Outcome::Available,
            (true, [Diagnostic::UnknownCompatibility { .. }]) => Outcome::AvailableWithWarning,
            (true, [Diagnostic::OverlaySkipped { .. }]) => Outcome::OverlaySkipped,
            (false, [Diagnostic::UnavailablePack { .. }]) => Outcome::Unavailable,
            other => panic!("{name}: unexpected {other:?}"),
        };
        assert!(
            about_pack.iter().all(|d| d.level() == Level::Warning),
            "{name}: {about_pack:?}"
        );
        assert_eq!(&observed, outcome, "{name}");
    }
}

#[test]
fn pack_metadata_shape_decides_availability() {
    assert_metadata_outcomes(&[
        (
            "an unknown feature makes the pack unavailable",
            Some(
                r#"{"pack":{"description":"x","min_format":122,"max_format":122},"features":{"enabled":["t:none"]}}"#,
            ),
            Outcome::Unavailable,
        ),
        ("invalid JSON", Some("{"), Outcome::Unavailable),
        (
            "a root that is not an object",
            Some("[]"),
            Outcome::Unavailable,
        ),
        (
            "no pack section",
            Some(r#"{"filter":{"block":[]}}"#),
            Outcome::Unavailable,
        ),
        (
            "an invalid description",
            Some(r#"{"pack":{"description":null,"min_format":122,"max_format":122}}"#),
            Outcome::Unavailable,
        ),
        ("a zip without metadata", None, Outcome::Unavailable),
        (
            "unknown fields are ignored",
            Some(
                r#"{"pack":{"description":"x","min_format":122,"max_format":122,"zzz":1},"zzz":{}}"#,
            ),
            Outcome::Available,
        ),
        (
            "a repeated key fails to parse",
            Some(
                r#"{"pack":{"description":"x","min_format":122,"max_format":122},"pack":{"description":"y"}}"#,
            ),
            Outcome::Unavailable,
        ),
        (
            "a features section that is not a list",
            Some(
                r#"{"pack":{"description":"x","min_format":122,"max_format":122},"features":{"enabled":3}}"#,
            ),
            Outcome::Unavailable,
        ),
        (
            "an overlay directory with a slash",
            Some(
                r#"{"pack":{"description":"x","min_format":122,"max_format":122},"overlays":{"entries":[{"directory":"a/b","min_format":122,"max_format":122}]}}"#,
            ),
            Outcome::Unavailable,
        ),
        (
            "an overlay without a format is skipped",
            Some(
                r#"{"pack":{"description":"x","min_format":122,"max_format":122},"overlays":{"entries":[{"directory":"o"}]}}"#,
            ),
            Outcome::OverlaySkipped,
        ),
        (
            "an overlay with only legacy formats applies",
            Some(
                r#"{"pack":{"description":"x","min_format":122,"max_format":122},"overlays":{"entries":[{"directory":"o","formats":122}]}}"#,
            ),
            Outcome::Available,
        ),
    ]);
}

#[test]
fn pack_format_range_decides_compatibility_warning() {
    assert_metadata_outcomes(&[
        (
            "an out of range pack is still available",
            Some(r#"{"pack":{"description":"x","min_format":[100],"max_format":[110]}}"#),
            Outcome::Available,
        ),
        (
            "a minimum without a maximum warns",
            Some(r#"{"pack":{"description":"x","min_format":[122,1]}}"#),
            Outcome::AvailableWithWarning,
        ),
        (
            "a legacy range above 81 warns",
            Some(
                r#"{"pack":{"description":"x","min_format":122,"max_format":122,"supported_formats":[1,200]}}"#,
            ),
            Outcome::AvailableWithWarning,
        ),
        (
            "a minimum above the maximum warns",
            Some(r#"{"pack":{"description":"x","min_format":122,"max_format":100}}"#),
            Outcome::AvailableWithWarning,
        ),
        (
            "a legacy format alone is available",
            Some(r#"{"pack":{"description":"x","pack_format":15}}"#),
            Outcome::Available,
        ),
        (
            "a format above 81 alone warns",
            Some(r#"{"pack":{"description":"x","pack_format":100}}"#),
            Outcome::AvailableWithWarning,
        ),
        (
            "a range reaching 81 without legacy fields warns",
            Some(r#"{"pack":{"description":"x","min_format":80,"max_format":122}}"#),
            Outcome::AvailableWithWarning,
        ),
        (
            "a range reaching 81 with legacy fields is available",
            Some(
                r#"{"pack":{"description":"x","min_format":80,"max_format":122,"pack_format":90,"supported_formats":[80,122]}}"#,
            ),
            Outcome::Available,
        ),
        (
            "a pack format outside the range warns",
            Some(
                r#"{"pack":{"description":"x","min_format":100,"max_format":110,"pack_format":120}}"#,
            ),
            Outcome::AvailableWithWarning,
        ),
    ]);
}

#[test]
fn known_packs_are_offered_in_stack_order_for_built_in_packs_only() {
    let trade = fs::read(vanilla_dir().join("datapacks/trade_rebalance/pack.mcmeta")).unwrap();
    let (stack, _) = pack_stack(
        &selection(&["vanilla", "file/x", "trade_rebalance"], &[]),
        &[
            AvailablePack::vanilla(),
            AvailablePack::directory("x", META),
            AvailablePack::feature("trade_rebalance", trade),
        ],
        &features(None),
        false,
    );
    let offered: Vec<&KnownPack> = stack.known_packs().collect();
    let expected = [
        KnownPack {
            namespace: "minecraft".into(),
            id: "core".into(),
            version: VERSION.id.clone(),
        },
        KnownPack {
            namespace: "minecraft".into(),
            id: "trade_rebalance".into(),
            version: VERSION.id.clone(),
        },
    ];
    assert_eq!(offered, expected.iter().collect::<Vec<_>>());
}
