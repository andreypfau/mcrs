use mcrs_minecraft_nbt_path::{NbtPath, NbtPathError};

#[test]
fn loot_mod_047_malformed_paths_fail_at_their_offset() {
    use NbtPathError::*;
    let cases: &[(&str, NbtPathError)] = &[
        ("", ExpectedNode { position: 0 }),
        ("a.", ExpectedNode { position: 2 }),
        ("a..b", ExpectedNode { position: 2 }),
        ("a.{}", ExpectedNode { position: 2 }),
        ("a b", ExpectedSeparator { position: 1 }),
        ("a ", ExpectedSeparator { position: 1 }),
        ("a[0] ", ExpectedSeparator { position: 4 }),
        ("{} ", ExpectedSeparator { position: 2 }),
        ("{}a", ExpectedSeparator { position: 2 }),
        ("a]", ExpectedSeparator { position: 1 }),
        (r#""a"b"#, ExpectedSeparator { position: 3 }),
        (r#""a"#, UnterminatedQuote { position: 2 }),
        (r#""a\"#, UnterminatedQuote { position: 3 }),
        (r#""a\x""#, InvalidEscape { position: 3 }),
        (r#"'a\"'"#, InvalidEscape { position: 3 }),
        ("a[x]", ExpectedIndex { position: 2 }),
        ("a[-]", ExpectedIndex { position: 2 }),
        ("a[ 1]", ExpectedIndex { position: 2 }),
        ("a[+1]", ExpectedIndex { position: 2 }),
        ("a[1.0]", ExpectedIndex { position: 2 }),
        ("a[1-]", ExpectedIndex { position: 2 }),
        ("a[--1]", ExpectedIndex { position: 2 }),
        ("a[.]", ExpectedIndex { position: 2 }),
        ("a[2147483648]", IndexOutOfRange { position: 2 }),
        ("a[-2147483649]", IndexOutOfRange { position: 2 }),
        ("a[1", ExpectedClosingBracket { position: 3 }),
        ("a[1x]", ExpectedClosingBracket { position: 3 }),
        (r#""""#, EmptyName { position: 0 }),
        ("a.''", EmptyName { position: 2 }),
        ("a{}.''{}", EmptyName { position: 4 }),
        ("a[{}", ExpectedClosingBracket { position: 4 }),
    ];
    for (text, expected) in cases {
        let result: Result<NbtPath, NbtPathError> = text.parse();
        assert_eq!(result, Err(*expected), "{text:?}");
    }
}

#[test]
fn loot_mod_047_a_pattern_that_is_not_snbt_fails() {
    for text in ["a{b:}", "{", "[{x:]"] {
        let result: Result<NbtPath, NbtPathError> = text.parse();
        assert!(
            matches!(result, Err(NbtPathError::Pattern(_))),
            "{text:?}: {result:?}"
        );
    }
}
