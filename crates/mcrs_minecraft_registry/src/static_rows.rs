use mcrs_minecraft_core::resource_location::ResourceLocation;

type Location = ResourceLocation<&'static str>;

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

pub const fn rows_match(rows: &[(&str, u16)], names: &[Location], complete: bool) -> bool {
    if complete && rows.len() != names.len() {
        return false;
    }
    let mut i = 0;
    while i < rows.len() {
        let (name, id) = rows[i];
        if id as usize >= names.len() || !same_name(name, names[id as usize].as_static_str()) {
            return false;
        }
        if complete && id as usize != i {
            return false;
        }
        i += 1;
    }
    true
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

    use mcrs_minecraft_core::rl;

    const NAMES: &[Location] = &[
        rl!("minecraft:air"),
        rl!("minecraft:stone"),
        rl!("minecraft:dirt"),
    ];

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
        let table: &[(&str, &[(&str, u16)], &[Location], bool, bool)] = &[
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
    fn names_compare_as_whole_text() {
        assert!(same_name("minecraft:air", "minecraft:air"));
        assert!(!same_name("minecraft:air", "minecraft:ai"));
        assert!(!same_name("minecraft:air", "minecraft:aix"));
        assert!(same_name("", ""));
    }
}
