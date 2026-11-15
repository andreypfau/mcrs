use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt_path::NbtPathOperationError;

use crate::{list, path, tag};

#[test]
fn loot_mod_050_get_or_create_creates_the_missing_tags() {
    let cases: &[(&str, &str, &str, &str, &str)] = &[
        ("a.b", "{}", "[]", "{a: {b: []}}", "[[]]"),
        ("a.b", "{a: {b: 7}}", "[]", "{a: {b: 7}}", "[7]"),
        ("a[]", "{a: []}", "{}", "{a: [{}]}", "[{}]"),
        ("a[]", "{a: [1, 2]}", "{}", "{a: [1, 2]}", "[1, 2]"),
        ("a[].b", "{}", "1", "{a: [{b: 1}]}", "[1]"),
        ("a[-1]", "{a: [1, 2]}", "{}", "{a: [1, 2]}", "[2]"),
        (
            "l[{x: 9}]",
            "{l: [{x: 1}]}",
            "[]",
            "{l: [{x: 1}, {x: 9}]}",
            "[{x: 9}]",
        ),
        (
            "l[{x: 1}]",
            "{l: [{x: 1, y: 2}]}",
            "[]",
            "{l: [{x: 1, y: 2}]}",
            "[{x: 1, y: 2}]",
        ),
        ("a{k: 1}", "{}", "[]", "{a: {k: 1}}", "[{k: 1}]"),
        ("{}.a", "{}", "5", "{a: 5}", "[5]"),
        ("a[0]", "{}", "{}", "{a: []}", "[]"),
        ("a.b[0]", "{}", "{}", "{a: {b: []}}", "[]"),
        ("a[2]", "{a: [1]}", "{}", "{a: [1]}", "[]"),
        ("a{k: 1}", "{a: {k: 2}}", "{}", "{a: {k: 2}}", "[]"),
        ("a.b", "{a: 5}", "{}", "{a: 5}", "[]"),
        ("a[]", "{a: {}}", "{}", "{a: {}}", "[]"),
        ("arr[]", "{arr: [I; 1, 2]}", "{}", "{arr: [I; 1, 2]}", "[]"),
        ("arr[0]", "{arr: [I; 1, 2]}", "{}", "{arr: [I; 1, 2]}", "[]"),
        ("arr[]", "{arr: [I;]}", "{}", "{arr: [I;]}", "[]"),
        ("arr[].x", "{arr: [I; 1]}", "{}", "{arr: [I; 1]}", "[]"),
    ];
    for (text, root, new_tag, expected_root, expected_tags) in cases {
        let mut root = tag(root);
        let mut tags = Vec::new();
        let result = path(text).get_or_create(&mut root, &tag(new_tag), &mut tags);
        assert_eq!(result, Ok(()), "{text}");
        let tags: Vec<NbtTag> = tags.into_iter().map(|tag| tag.clone()).collect();
        assert_eq!(tags, list(expected_tags), "{text}");
        assert_eq!(root, tag(expected_root), "{text}");
    }
}

#[test]
fn loot_mod_050_get_or_create_fails_when_a_node_before_the_last_gives_no_tag() {
    let cases: &[(&str, &str, &str)] = &[
        ("x[0].y", "{}", "{x: []}"),
        ("{z: 1}.a", "{}", "{}"),
        ("a.b.c", "{a: 5}", "{a: 5}"),
        ("a{k: 1}.b", "{a: {k: 2}}", "{a: {k: 2}}"),
        ("arr[].x", "{arr: [I;]}", "{arr: [I;]}"),
        ("arr[].x.y", "{arr: [I; 1]}", "{arr: [I; 1]}"),
    ];
    for (text, root, expected_root) in cases {
        let mut root = tag(root);
        let mut tags = Vec::new();
        let result = path(text).get_or_create(&mut root, &tag("{}"), &mut tags);
        assert_eq!(result, Err(NbtPathOperationError::NoTag), "{text}");
        assert_eq!(root, tag(expected_root), "{text}");
    }
}

#[test]
fn loot_mod_051_set_writes_the_value_at_the_last_node() {
    use NbtPathOperationError::*;
    let cases: &[(&str, &str, Result<(), NbtPathOperationError>, &str)] = &[
        ("a", "{a: 1}", Ok(()), "{a: 9}"),
        ("a.b", "{}", Ok(()), "{a: {b: 9}}"),
        ("a.b.c", "{}", Ok(()), "{a: {b: {c: 9}}}"),
        ("a.b[]", "{}", Ok(()), "{a: {b: [9]}}"),
        ("a[1]", "{a: [1, 2]}", Ok(()), "{a: [1, 9]}"),
        ("a[-2]", "{a: [1, 2]}", Ok(()), "{a: [9, 2]}"),
        ("a[5]", "{a: [1, 2]}", Ok(()), "{a: [1, 2]}"),
        ("a[]", "{a: [1, 2]}", Ok(()), "{a: [9, 9]}"),
        ("a[]", "{a: []}", Ok(()), "{a: [9]}"),
        (
            "l[{x: 1}]",
            "{l: [{x: 1}, {x: 2}, {x: 1, y: 0}]}",
            Ok(()),
            "{l: [9, {x: 2}, 9]}",
        ),
        ("l[{x: 1}]", "{l: []}", Ok(()), "{l: [9]}"),
        ("l[{x: 1}]", "{l: [{x: 2}]}", Ok(()), "{l: [{x: 2}]}"),
        (
            "l[].x",
            "{l: [{x: 1}, {}]}",
            Ok(()),
            "{l: [{x: 9}, {x: 9}]}",
        ),
        ("a{k: 1}", "{a: {k: 1, m: 2}}", Ok(()), "{a: 9}"),
        ("a{k: 1}", "{a: {k: 2}}", Ok(()), "{a: {k: 2}}"),
        ("a{k: 1}", "{}", Ok(()), "{}"),
        ("{}", "{}", Ok(()), "{}"),
        ("a[0]", "{}", Ok(()), "{a: []}"),
        ("x[0].y", "{}", Err(NoTag), "{x: []}"),
        ("x[0].y.z", "{}", Err(NoTag), "{x: []}"),
        ("{z: 1}.a", "{}", Err(NoTag), "{}"),
        ("arr[]", "{arr: [I; 1, 2]}", Ok(()), "{arr: [I; 9, 9]}"),
        ("arr[]", "{arr: [B;]}", Ok(()), "{arr: [B; 9b]}"),
        ("arr[-1]", "{arr: [L; 1, 2]}", Ok(()), "{arr: [L; 1l, 9l]}"),
        ("arr[1]", "{arr: [I; 1]}", Ok(()), "{arr: [I; 1]}"),
        ("arr[{}]", "{arr: [I; 1]}", Ok(()), "{arr: [I; 1]}"),
        ("arr[].x", "{arr: [I; 1]}", Ok(()), "{arr: [I; 1]}"),
    ];
    for (text, root, expected, expected_root) in cases {
        let mut root = tag(root);
        let result = path(text).set(&mut root, &tag("9"));
        assert_eq!(result, *expected, "{text}");
        assert_eq!(root, tag(expected_root), "{text}");
    }
}

#[test]
fn loot_mod_051_set_converts_the_value_to_the_element_type_of_an_array() {
    let cases: &[(&str, NbtTag, &str)] = &[
        ("[B; 0b]", NbtTag::Int(300), "[B; 44b]"),
        ("[B; 0b]", NbtTag::Short(-129), "[B; 127b]"),
        ("[B; 0b]", NbtTag::Double(200.5), "[B; 127b]"),
        ("[B; 0b]", NbtTag::Float(-1.5), "[B; -2b]"),
        ("[I; 0]", NbtTag::Long(4_294_967_297), "[I; 1]"),
        ("[I; 0]", NbtTag::Double(2.7), "[I; 2]"),
        ("[I; 0]", NbtTag::Float(-2.5), "[I; -3]"),
        ("[I; 0]", NbtTag::Double(1e10), "[I; 2147483647]"),
        ("[I; 0]", NbtTag::Double(f64::NAN), "[I; 0]"),
        (
            "[I; 0]",
            NbtTag::Double(f64::NEG_INFINITY),
            "[I; -2147483648]",
        ),
        ("[L; 0l]", NbtTag::Byte(-5), "[L; -5l]"),
        ("[L; 0l]", NbtTag::Double(1e30), "[L; 9223372036854775807l]"),
        (
            "[L; 0l]",
            NbtTag::Double(-1e30),
            "[L; -9223372036854775808l]",
        ),
        ("[L; 0l]", NbtTag::Float(-0.5), "[L; -1l]"),
    ];
    for (array, value, expected) in cases {
        let mut root = NbtTag::List(vec![tag(array)]);
        assert_eq!(
            path("[0][0]").set(&mut root, value),
            Ok(()),
            "{array} {value:?}"
        );
        assert_eq!(root, NbtTag::List(vec![tag(expected)]), "{array} {value:?}");
    }
}

#[test]
fn loot_mod_051_a_value_that_is_not_numeric_leaves_the_array_unchanged() {
    for (text, array) in [("a[]", "[I; 1, 2]"), ("a[]", "[I;]"), ("a[0]", "[B; 1b]")] {
        for value in ["\"x\"", "{}", "[1]"] {
            let mut root = NbtTag::Compound(NbtCompound::new());
            if let NbtTag::Compound(compound) = &mut root {
                compound.put("a", tag(array));
            }
            let before = root.clone();
            let result = path(text).set(&mut root, &tag(value));
            assert_eq!(result, Ok(()), "{text} {array} {value}");
            assert_eq!(root, before, "{text} {array} {value}");
        }
    }
}

#[test]
fn loot_mod_051_set_fails_before_any_change_when_nodes_plus_level_reach_512() {
    let nested = |level: usize, innermost: NbtTag| {
        let mut value = innermost;
        for _ in 0..level {
            value = NbtTag::List(vec![value]);
        }
        value
    };
    let cases: &[(&str, usize, NbtTag, Result<(), NbtPathOperationError>)] = &[
        ("a", 510, NbtTag::Int(1), Ok(())),
        (
            "a",
            511,
            NbtTag::Int(1),
            Err(NbtPathOperationError::TooDeep),
        ),
        ("a", 510, NbtTag::IntArray(vec![1]), Ok(())),
        ("a.b", 509, NbtTag::Int(1), Ok(())),
        (
            "a.b",
            510,
            NbtTag::Int(1),
            Err(NbtPathOperationError::TooDeep),
        ),
    ];
    for (text, level, innermost, expected) in cases {
        let mut root = tag("{}");
        let result = path(text).set(&mut root, &nested(*level, innermost.clone()));
        assert_eq!(result, *expected, "{text} at level {level}");
        if expected.is_err() {
            assert_eq!(root, tag("{}"), "{text} at level {level}");
        }
    }
}
