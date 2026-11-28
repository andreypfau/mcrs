use mcrs_minecraft_swing::SwingModifiers;
use mcrs_minecraft_swing::SwingTracker;

#[test]
fn haste_conduit_and_mining_fatigue_scale_duration() {
    let cases: [(&str, &[(&str, u8)], i32, i32); 15] = [
        ("no effects", &[], 6, 6),
        ("negative base clamps to zero", &[], -4, 0),
        ("haste 0", &[("minecraft:haste", 0)], 6, 5),
        ("unqualified haste 2", &[("haste", 2)], 10, 7),
        ("haste clamps to zero", &[("haste", 2)], 2, 0),
        ("conduit power 3", &[("minecraft:conduit_power", 3)], 10, 6),
        (
            "haste overrides fatigue",
            &[("minecraft:mining_fatigue", 4), ("minecraft:haste", 0)],
            6,
            5,
        ),
        (
            "conduit overrides fatigue",
            &[
                ("minecraft:mining_fatigue", 4),
                ("minecraft:conduit_power", 0),
            ],
            6,
            5,
        ),
        (
            "conduit beats weaker haste",
            &[("minecraft:haste", 1), ("minecraft:conduit_power", 2)],
            10,
            7,
        ),
        (
            "haste beats weaker conduit",
            &[("minecraft:haste", 10), ("minecraft:conduit_power", 2)],
            24,
            13,
        ),
        (
            "duplicate haste keeps max amplifier",
            &[("minecraft:haste", 5), ("minecraft:haste", 2)],
            10,
            4,
        ),
        ("foreign namespace ignored", &[("other:haste", 5)], 6, 6),
        ("fatigue 1", &[("minecraft:mining_fatigue", 1)], 6, 10),
        ("fatigue 3", &[("minecraft:mining_fatigue", 3)], 6, 14),
        ("fatigue 4", &[("minecraft:mining_fatigue", 4)], 6, 16),
    ];

    for (label, effects, base_duration, expected) in cases {
        let modifiers = SwingModifiers::from_effects(effects.iter().copied());
        assert_eq!(
            modifiers.effective_duration(base_duration),
            expected,
            "SWG-STA-002 ({label})"
        );
    }
}

#[test]
fn tracker_enforces_same_tick_and_half_duration_cooldown() {
    let none = SwingModifiers::default();
    let haste2 = SwingModifiers::from_effects([("minecraft:haste", 2)]);
    let haste20 = SwingModifiers::from_effects([("minecraft:haste", 20)]);
    let fatigue1 = SwingModifiers::from_effects([("minecraft:mining_fatigue", 1)]);
    let fatigue4 = SwingModifiers::from_effects([("minecraft:mining_fatigue", 4)]);

    let cases: [(&str, i32, SwingModifiers, i32, u64); 9] = [
        ("D=0", 0, none, 0, 1),
        ("D=1", 1, none, 1, 1),
        ("D=2", 2, none, 2, 2),
        ("D=5", 5, none, 5, 3),
        ("D=6", 6, none, 6, 4),
        ("D=7 from B=10 haste 2", 10, haste2, 7, 4),
        ("D=10 from B=6 fatigue 1", 6, fatigue1, 10, 6),
        ("D=12", 12, none, 12, 7),
        ("D=16 from B=6 fatigue 4", 6, fatigue4, 16, 9),
    ];

    for (label, base_duration, modifiers, expected_scaled, unlock_offset) in cases {
        let mut tracker = SwingTracker::default();
        let start = 100;
        assert_eq!(
            tracker.try_start(start, base_duration, modifiers),
            Some(expected_scaled),
            "SWG-STA-003 start ({label})"
        );
        assert_eq!(
            tracker.try_start(start - 1, base_duration, none),
            None,
            "SWG-STA-004 earlier tick blocked ({label})"
        );
        for offset in 0..unlock_offset {
            assert_eq!(
                tracker.try_start(start + offset, base_duration, none),
                None,
                "SWG-STA-003/004 blocked at T+{offset} ({label})"
            );
        }
        assert_eq!(
            tracker.try_start(start + unlock_offset, base_duration, none),
            Some(base_duration),
            "SWG-STA-003 unlocks at T+{unlock_offset} ({label})"
        );
    }

    let mut midwindow = SwingTracker::default();
    assert_eq!(midwindow.try_start(100, 10, none), Some(10));
    assert_eq!(
        midwindow.try_start(102, 10, haste20),
        None,
        "SWG-STA-003: later haste must not shorten an active window"
    );
    assert_eq!(
        midwindow.try_start(105, 10, haste20),
        None,
        "SWG-STA-003: D=10 stays blocked at T+5 even with haste 20"
    );
    assert_eq!(
        midwindow.try_start(106, 10, haste20),
        Some(0),
        "SWG-STA-003: D=10 unlocks at T+6"
    );
}
