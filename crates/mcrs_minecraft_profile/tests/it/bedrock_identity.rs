use std::collections::HashSet;

use mcrs_minecraft_profile::{
    BedrockNameError, BedrockNamePolicy, BedrockUuidScheme, PlayerName, Xuid,
};
use uuid::{Uuid, Variant};

const SCHEMES: [BedrockUuidScheme; 2] = [
    BedrockUuidScheme::FloodgateCompatible,
    BedrockUuidScheme::Rfc9562,
];

#[test]
fn both_schemes_round_trip_an_xuid() {
    for xuid in [1, 2_535_412_345_678_901, 0x7fff_ffff_ffff_ffff, u64::MAX].map(Xuid) {
        for scheme in SCHEMES {
            let uuid = scheme.to_uuid(xuid);
            assert_eq!(scheme.xuid(uuid), Some(xuid), "{scheme:?} {xuid}");
            assert_eq!(
                BedrockUuidScheme::detect(uuid),
                Some((xuid, scheme)),
                "{scheme:?} {xuid}"
            );
        }
        let rfc = BedrockUuidScheme::Rfc9562.to_uuid(xuid);
        assert_eq!(
            (rfc.get_version_num(), rfc.get_variant()),
            (8, Variant::RFC4122)
        );
    }
}

#[test]
fn no_foreign_uuid_decodes_as_an_xuid() {
    let java = [
        "4566e69f-c907-48ee-8d71-d7ba5aa00c20",
        "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "c9f0ef72-466d-357d-bc67-4d924d54da80",
        "00000000-0000-0000-0000-000000000000",
        "6d637274-0000-8000-8000-000000000001",
        "6d637273-0000-8000-c000-000000000001",
    ];
    for uuid in java.map(|text| Uuid::parse_str(text).unwrap()) {
        assert_eq!(BedrockUuidScheme::detect(uuid), None, "{uuid}");
    }
}

#[test]
fn online_gamertags_that_cut_to_one_name_get_different_names() {
    let policy = BedrockNamePolicy::default();
    let mut online = HashSet::new();
    let mut names = Vec::new();
    for tag in [
        "Super Long Gamertag A",
        "Super Long Gamertag B",
        "Super Long Game1",
        "Super Long Gamertag C",
    ] {
        let name = policy.java_name(tag, &online).unwrap();
        online.insert(name.clone());
        names.push(name.0);
    }
    assert_eq!(
        names,
        [
            ".Super_Long_Game",
            ".Super_Long_Gam1",
            ".Super_Long_Gam2",
            ".Super_Long_Gam3"
        ]
    );
}

#[test]
fn a_policy_that_cannot_give_a_name_is_an_error() {
    let online: HashSet<PlayerName> = [PlayerName::new("A").unwrap()].into();
    let cases = [
        (None, 1, Err(BedrockNameError::NoFreeName)),
        (None, 0, Err(BedrockNameError::Empty)),
        (Some(' '), 16, Err(BedrockNameError::InvalidPolicy)),
    ];
    for (prefix, max_length, expected) in cases {
        let policy = BedrockNamePolicy {
            prefix,
            space_replacement: None,
            max_length,
        };
        assert_eq!(
            policy.java_name("A", &online),
            expected,
            "{prefix:?} {max_length}"
        );
    }
}

#[test]
fn unrelated_players_do_not_shorten_collision_names() {
    for (max_length, gamertag, expected) in [(2, "A", "A1"), (4, "ABCD", "ABC1")] {
        let policy = BedrockNamePolicy {
            prefix: None,
            space_replacement: None,
            max_length,
        };
        let mut online: HashSet<PlayerName> = (0..99)
            .map(|index| PlayerName::new(format!("P{index}")).unwrap())
            .collect();
        online.insert(PlayerName::new(gamertag).unwrap());
        assert_eq!(
            policy.java_name(gamertag, &online),
            Ok(PlayerName::new(expected).unwrap()),
            "maximum length {max_length}"
        );
    }
}

#[test]
fn collision_suffixes_cross_a_digit_boundary_and_report_exhaustion() {
    for (max_length, gamertag, occupied, expected) in [
        (3, "A11", 10..20, Ok(PlayerName::new("A20").unwrap())),
        (2, "A", 1..10, Err(BedrockNameError::NoFreeName)),
    ] {
        let policy = BedrockNamePolicy {
            prefix: None,
            space_replacement: None,
            max_length,
        };
        let mut online: HashSet<PlayerName> = occupied
            .map(|index| PlayerName::new(format!("A{index}")).unwrap())
            .collect();
        online.insert(PlayerName::new(gamertag).unwrap());
        assert_eq!(policy.java_name(gamertag, &online), expected);
    }
}
