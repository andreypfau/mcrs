pub const fn same_name(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

pub const fn rows_match(rows: &[(&str, u16)], names: &[&str], complete: bool) -> bool {
    if complete && rows.len() != names.len() {
        return false;
    }
    let mut i = 0;
    while i < rows.len() {
        let (name, id) = rows[i];
        if id as usize >= names.len() || !same_name(name, names[id as usize]) {
            return false;
        }
        if complete && id as usize != i {
            return false;
        }
        i += 1;
    }
    true
}

const fn count_of(list: &[&str], name: &str) -> usize {
    let mut count = 0;
    let mut i = 0;
    while i < list.len() {
        if same_name(list[i], name) {
            count += 1;
        }
        i += 1;
    }
    count
}

pub const fn names_cover(rows: &[&str], unsupported: &[&str], names: &[&str]) -> bool {
    let mut i = 0;
    while i < names.len() {
        if count_of(rows, names[i]) + count_of(unsupported, names[i]) != 1 {
            return false;
        }
        i += 1;
    }
    rows.len() + unsupported.len() == names.len()
}

#[cfg(feature = "test-support")]
pub fn assert_dispatch<T: serde::de::DeserializeOwned>(
    rows: &[&str],
    unsupported: &[&str],
    names: &[&str],
    probe: impl Fn(&str) -> serde_json::Value,
) {
    let unknown = |name: &str| {
        serde_json::from_value::<T>(probe(name))
            .err()
            .map(|e| e.to_string())
            .filter(|e| e.contains("unknown variant"))
    };
    let message =
        unknown("mcrs:no_such_entry").expect("an unknown tag is refused as an unknown variant");
    let declared: Vec<&str> = message.split('`').skip(1).step_by(2).skip(1).collect();
    for row in rows {
        assert!(declared.contains(row), "{row} has no variant");
        assert!(
            unknown(row).is_none(),
            "{row} is refused as an unknown variant"
        );
    }
    for name in unsupported {
        assert!(
            !declared.contains(name),
            "{name} is listed unsupported but has a variant"
        );
        assert!(
            unknown(name).is_some(),
            "{name} is listed unsupported but is accepted"
        );
    }
    for variant in declared {
        assert!(
            rows.contains(&variant) || !names.contains(&variant),
            "{variant} names a registry entry that is not in the rows"
        );
    }
}

pub const fn numbered<const N: usize>(names: [&'static str; N]) -> [(&'static str, u16); N] {
    let mut rows = [("", 0); N];
    let mut i = 0;
    while i < N {
        rows[i] = (names[i], i as u16);
        i += 1;
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAMES: &[&str] = &["minecraft:air", "minecraft:stone", "minecraft:dirt"];

    #[test]
    fn numbered_rows_carry_their_position() {
        const ROWS: [(&str, u16); 3] =
            numbered(["minecraft:air", "minecraft:stone", "minecraft:dirt"]);
        assert_eq!(
            ROWS,
            [
                ("minecraft:air", 0),
                ("minecraft:stone", 1),
                ("minecraft:dirt", 2)
            ]
        );
        assert!(rows_match(&ROWS, NAMES, true));
    }

    #[test]
    fn the_row_check_refuses_every_drift() {
        let table: &[(&str, &[(&str, u16)], &[&str], bool, bool)] = &[
            (
                "complete table",
                &[
                    ("minecraft:air", 0),
                    ("minecraft:stone", 1),
                    ("minecraft:dirt", 2),
                ],
                NAMES,
                true,
                true,
            ),
            (
                "missing first row",
                &[("minecraft:stone", 1), ("minecraft:dirt", 2)],
                NAMES,
                true,
                false,
            ),
            (
                "missing last row",
                &[("minecraft:air", 0), ("minecraft:stone", 1)],
                NAMES,
                true,
                false,
            ),
            (
                "swapped rows",
                &[
                    ("minecraft:air", 0),
                    ("minecraft:dirt", 1),
                    ("minecraft:stone", 2),
                ],
                NAMES,
                true,
                false,
            ),
            (
                "extra row",
                &[
                    ("minecraft:air", 0),
                    ("minecraft:stone", 1),
                    ("minecraft:dirt", 2),
                    ("minecraft:grass", 3),
                ],
                NAMES,
                true,
                false,
            ),
            (
                "row without namespace",
                &[("air", 0), ("minecraft:stone", 1), ("minecraft:dirt", 2)],
                NAMES,
                true,
                false,
            ),
            (
                "wire id off by one",
                &[
                    ("minecraft:air", 0),
                    ("minecraft:stone", 2),
                    ("minecraft:dirt", 2),
                ],
                NAMES,
                true,
                false,
            ),
            (
                "wire id past the registry",
                &[
                    ("minecraft:air", 0),
                    ("minecraft:stone", 1),
                    ("minecraft:dirt", 3),
                ],
                NAMES,
                true,
                false,
            ),
            ("empty against empty", &[], &[], true, true),
            ("empty against a registry", &[], NAMES, true, false),
            (
                "rows against an empty registry",
                &[("minecraft:air", 0)],
                &[],
                true,
                false,
            ),
            (
                "partial mirror in any order",
                &[("minecraft:dirt", 2), ("minecraft:air", 0)],
                NAMES,
                false,
                true,
            ),
            ("partial empty mirror", &[], NAMES, false, true),
            (
                "partial mirror with a wrong name",
                &[("minecraft:dirt", 1)],
                NAMES,
                false,
                false,
            ),
            (
                "partial mirror past the registry",
                &[("minecraft:dirt", 3)],
                NAMES,
                false,
                false,
            ),
        ];
        for (case, rows, names, complete, expected) in table {
            assert_eq!(rows_match(rows, names, *complete), *expected, "{case}");
        }
    }

    #[test]
    fn the_cover_check_refuses_every_drift() {
        let table: &[(&str, &[&str], &[&str], bool)] = &[
            (
                "rows and unsupported split the registry",
                &["minecraft:air", "minecraft:dirt"],
                &["minecraft:stone"],
                true,
            ),
            ("rows alone", NAMES, &[], true),
            ("unsupported alone", &[], NAMES, true),
            (
                "name in neither list",
                &["minecraft:air"],
                &["minecraft:stone"],
                false,
            ),
            (
                "name in both lists",
                &["minecraft:air", "minecraft:stone", "minecraft:dirt"],
                &["minecraft:stone"],
                false,
            ),
            (
                "name twice in rows",
                &[
                    "minecraft:air",
                    "minecraft:air",
                    "minecraft:stone",
                    "minecraft:dirt",
                ],
                &[],
                false,
            ),
            (
                "row outside the registry",
                &[
                    "minecraft:air",
                    "minecraft:stone",
                    "minecraft:dirt",
                    "minecraft:grass",
                ],
                &[],
                false,
            ),
            (
                "unsupported outside the registry",
                &["minecraft:air", "minecraft:stone", "minecraft:dirt"],
                &["minecraft:grass"],
                false,
            ),
            (
                "row without namespace",
                &["air", "minecraft:stone", "minecraft:dirt"],
                &[],
                false,
            ),
            ("empty against a registry", &[], &[], false),
        ];
        for (case, rows, unsupported, expected) in table {
            assert_eq!(names_cover(rows, unsupported, NAMES), *expected, "{case}");
        }
        assert!(names_cover(&[], &[], &[]), "empty against empty");
    }

    #[test]
    fn names_compare_as_whole_text() {
        assert!(same_name("minecraft:air", "minecraft:air"));
        assert!(!same_name("minecraft:air", "minecraft:ai"));
        assert!(!same_name("minecraft:air", "minecraft:aix"));
        assert!(same_name("", ""));
    }
}
