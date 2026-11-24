use crate::{list, path, selected, tag};

const ROOT: &str = r#"{
    a: {b: [1, 2, 3], c: {d: 1b}},
    l: [{x: 1}, {x: 2, y: 3}, 5],
    arr: [I; 7, 8, 9],
    "q k": 4,
    "a\"b": 6,
    e: []
}"#;

#[test]
fn loot_mod_047_048_each_node_form_selects_its_tags() {
    let root = tag(ROOT);
    let cases: &[(&str, &str)] = &[
        ("a.c.d", "[1b]"),
        ("a.b[]", "[1, 2, 3]"),
        ("a.b[0]", "[1]"),
        ("a.b.[0]", "[1]"),
        ("a.b[-1]", "[3]"),
        ("a.b[-3]", "[1]"),
        ("a.b[3]", "[]"),
        ("a.b[-4]", "[]"),
        ("a.b[-0]", "[1]"),
        ("a.b[2147483647]", "[]"),
        ("a.b[-2147483648]", "[]"),
        ("a.b[002]", "[3]"),
        ("a.missing", "[]"),
        ("a.c.d.e", "[]"),
        ("l[{x: 2}]", "[{x: 2, y: 3}]"),
        ("l[{}]", "[{x: 1}, {x: 2, y: 3}]"),
        ("l[]", "[{x: 1}, {x: 2, y: 3}, 5]"),
        ("l[].x", "[1, 2]"),
        ("l[1].y", "[3]"),
        ("arr[]", "[7, 8, 9]"),
        ("arr[-3]", "[7]"),
        ("arr[{}]", "[]"),
        ("[]", "[]"),
        ("[0]", "[]"),
        (r#""q k""#, "[4]"),
        ("'q k'", "[4]"),
        (r#""a\"b""#, "[6]"),
        (r#"'a"b'"#, "[6]"),
        ("a{c: {}}.c.d", "[1b]"),
        ("a{c: {d: 2b}}", "[]"),
        ("{e: []}.a.c.d", "[1b]"),
        ("{e: []}[0]", "[]"),
        ("{e: [1]}.a", "[]"),
        ("{nope: 1}.a.c.d", "[]"),
    ];
    for (text, expected) in cases {
        assert_eq!(selected(text, &root), list(expected), "{text}");
    }
}

#[test]
fn loot_mod_048_root_node_selects_the_root_itself() {
    let root = tag("{k: 1}");
    assert_eq!(selected("{k: 1}", &root), vec![root]);
}

#[test]
fn loot_mod_049_match_compares_lists_by_membership_and_length() {
    let cases: &[(&str, &str, bool)] = &[
        ("[1, 1]", "[1]", false),
        ("[1, 1]", "[1, 2]", true),
        ("[1]", "[2, 1, 1]", true),
        ("[]", "[1]", false),
        ("[]", "[]", true),
        ("[1]", "[I; 1]", false),
        ("[I; 1, 2]", "[I; 1, 2]", true),
        ("[I; 1]", "[I; 1, 2]", false),
        ("1", "1b", false),
        ("{a: 1}", "{a: 1, b: 2}", true),
        ("{a: 1, c: 1}", "{a: 1}", false),
        ("[{a: 1}]", "[{a: 1, b: 2}]", true),
    ];
    for (pattern, value, matches) in cases {
        let root = tag(&format!("{{v: {value}}}"));
        let selected = selected(&format!("{{v: {pattern}}}"), &root);
        let expected = if *matches {
            vec![root.clone()]
        } else {
            Vec::new()
        };
        assert_eq!(selected, expected, "{pattern} against {value}");
    }
}

#[test]
fn cmd_data_004_a_selection_stops_at_the_end_of_the_node_that_selects_nothing() {
    let root = tag("{Pos: [0.5d, -10.7d, 0.5d], NoGravity: 1b, Tags: [\"a\", \"b\"]}");
    let cases: &[(&str, Option<&str>)] = &[
        ("Pos[0]", None),
        ("Tags[]", None),
        ("Pos[5].x", Some("Pos[5]")),
        ("a.b.c", Some("a")),
        ("{NoGravity: 0b}", Some("{NoGravity: 0b}")),
        ("{NoGravity: 1b}.Pos[1].x", Some("{NoGravity: 1b}.Pos[1].x")),
    ];
    for &(text, expected) in cases {
        let mut selected = Vec::new();
        let found = path(text).try_select(&root, &mut selected);
        let reported = found.err().map(|nothing| &text[..nothing.end]);
        assert_eq!(reported, expected, "{text}");
    }
}
