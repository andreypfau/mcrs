use std::borrow::ToOwned;
use std::format;
use std::io::Cursor;
use std::string::String;
use std::vec::Vec;

use mcrs_minecraft_nbt::tag::NbtTag;

use crate::build::Validate;
use crate::golden::GOLDEN;
use crate::reader::Reader;
use crate::*;

fn unhex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fn hex(tag: &NbtTag) -> String {
    let mut bytes = Vec::new();
    mcrs_minecraft_nbt::to_bytes_unnamed(tag, &mut bytes).unwrap();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn decode(bytes: &[u8]) -> NbtTag {
    mcrs_minecraft_nbt::from_bytes_unnamed(Cursor::new(bytes)).unwrap()
}

fn no_compound_has_two_keys(tag: &NbtTag) -> bool {
    match tag {
        NbtTag::Compound(compound) => {
            compound.child_tags.len() <= 1
                && compound
                    .child_tags
                    .iter()
                    .all(|(_, value)| no_compound_has_two_keys(value))
        }
        NbtTag::List(list) => list.iter().all(no_compound_has_two_keys),
        _ => true,
    }
}

/// Vanilla's compound is a hash map, so its bytes carry no key order; the
/// tag decoded from them is compared through the key-sorted SNBT form, and
/// the bytes are compared only where no compound has two keys.
#[test]
fn parses_like_vanilla() {
    for case in &GOLDEN.parses {
        let input = &case.input;
        let (_, end) = read_tag(input, 0).unwrap_or_else(|e| panic!("{input:?}: {e}"));
        assert_eq!(validate(input), Ok(end), "validation of {input:?}");
        let tag = parse_tag(input).unwrap_or_else(|e| panic!("{input:?}: {e}"));
        assert_eq!(write(&tag), case.snbt, "snbt of {input:?}");
        if no_compound_has_two_keys(&tag) {
            assert_eq!(hex(&tag), case.hex, "bytes written for {input:?}");
        }
        assert_eq!(
            write(&decode(&unhex(&case.hex))),
            case.snbt,
            "bytes of {input:?}"
        );
        assert_eq!(
            write(&parse_tag(&case.snbt).unwrap()),
            case.snbt,
            "round trip of {input:?}"
        );
    }
}

#[test]
fn rejects_like_vanilla() {
    for input in &GOLDEN.rejects {
        assert!(parse_tag(input).is_err(), "{input:?} should not parse");
    }
}

#[test]
fn formats_numbers_like_java() {
    for case in &GOLDEN.doubles {
        assert_eq!(java_double(f64::from_bits(case.bits)), case.text);
    }
    for case in &GOLDEN.floats {
        assert_eq!(java_float(f32::from_bits(case.bits)), case.text);
    }
}

#[test]
fn json_numbers_are_typed_like_json_ops() {
    for case in &GOLDEN.json_typing {
        let tag: NbtTag = serde_json::from_str(&case.json).unwrap();
        assert_eq!(write(&tag), case.tag, "{}", case.json);
    }
}

#[test]
fn writes_like_vanilla_and_round_trips_its_bytes() {
    let tag = decode(&unhex(&GOLDEN.pretty_hex));
    assert_eq!(write(&tag), GOLDEN.pretty);
    assert_eq!(hex(&tag), GOLDEN.pretty_hex);
}

fn validate(input: &str) -> Result<usize, SnbtError> {
    let mut reader = Reader::new(input, 0, Validate);
    reader.read().map(|_| reader.position())
}

fn rendered(input: &str, error: SnbtError) -> (usize, &'static str, String) {
    let mut argument = String::new();
    error.kind.write_argument(input, &mut argument).unwrap();
    (error.position, error.kind.key(), argument)
}

const LITERAL: &str = "argument.literal.incorrect";
const NUMBER_PARSE_FAILURE: &str = "snbt.parser.number_parse_failure";

const ERRORS: &[(&str, usize, &str, &str)] = &[
    ("{", 1, LITERAL, "\""),
    ("{,}", 1, LITERAL, "\""),
    ("{a}", 2, LITERAL, ":"),
    ("{a b:1}", 3, LITERAL, ":"),
    ("{a:}", 3, "snbt.parser.expected_unquoted_string", ""),
    ("{a:@}", 3, "snbt.parser.expected_unquoted_string", ""),
    ("{a:1", 4, LITERAL, "."),
    ("{a:1a}", 4, LITERAL, "."),
    ("{a:1 000}", 5, LITERAL, "."),
    ("{a:012}", 6, LITERAL, "."),
    ("{a:b", 4, LITERAL, "("),
    ("{a:true", 7, LITERAL, "("),
    ("{a:\"x\"", 6, LITERAL, ","),
    ("{a:1b b:2}", 6, LITERAL, ","),
    ("{a:1.5b}", 6, LITERAL, "e|E"),
    ("{a:1.e}", 6, LITERAL, "+"),
    ("{a:1e400}", 8, LITERAL, "f|F"),
    ("{a:1e400d}", 9, "snbt.parser.infinity_not_allowed", ""),
    ("{\"\":1}", 5, LITERAL, "."),
    ("{\"\":\"a\"}", 7, "snbt.parser.empty_key", ""),
    ("{a:-}", 4, "snbt.parser.expected_decimal_numeral", ""),
    (".", 1, "snbt.parser.expected_decimal_numeral", ""),
    ("{a:0x}", 5, "snbt.parser.expected_hex_numeral", ""),
    ("{a:0b2}", 5, "snbt.parser.expected_binary_numeral", ""),
    ("[,]", 1, LITERAL, "B"),
    ("[1,,2]", 3, "snbt.parser.expected_unquoted_string", ""),
    ("[1 2]", 3, LITERAL, "."),
    ("[B; 1.5]", 5, LITERAL, "u|U"),
    ("[B;01]", 5, "snbt.parser.leading_zero_not_allowed", ""),
    ("[B;1_]", 3, LITERAL, "+"),
    ("[B;0x1_]", 5, "snbt.parser.underscore_not_allowed", ""),
    ("[B;300]", 7, NUMBER_PARSE_FAILURE, ""),
    ("[B;1s]", 6, "snbt.parser.invalid_array_element_type", ""),
    (
        "[B;-1ub]",
        8,
        "snbt.parser.expected_non_negative_number",
        "",
    ),
    ("{a:128b}", 7, NUMBER_PARSE_FAILURE, ""),
    (
        "{a:-1ub}",
        7,
        "snbt.parser.expected_non_negative_number",
        "",
    ),
    ("\"abc", 4, "snbt.parser.invalid_string_contents", ""),
    ("\"\\q\"", 2, LITERAL, "b"),
    ("{a:\"\\x4\"}", 6, "snbt.parser.expected_hex_escape", "2"),
    ("\"\\x 41\"", 3, "snbt.parser.expected_hex_escape", "2"),
    (
        "{a:\"\\U00110000\"}",
        14,
        "snbt.parser.invalid_codepoint",
        "U+00110000",
    ),
    ("\"\\N{FOO}\"", 8, "snbt.parser.invalid_character_name", ""),
    ("-a()", 4, "snbt.parser.invalid_unquoted_start", ""),
    ("{a:foo(1)}", 9, "snbt.parser.no_such_operation", "foo/1"),
    ("{a:b (1)}", 8, "snbt.parser.no_such_operation", "b/1"),
    ("{a:bool()}", 9, "snbt.parser.no_such_operation", "bool/0"),
    (
        "{a:bool(\"x\")}",
        12,
        "snbt.parser.expected_number_or_boolean",
        "",
    ),
    (
        "{a:uuid(\"x\")}",
        12,
        "snbt.parser.expected_string_uuid",
        "",
    ),
    ("", 0, "snbt.parser.expected_unquoted_string", ""),
    ("   ", 3, "snbt.parser.expected_unquoted_string", ""),
    ("{foo=bar}", 4, LITERAL, ":"),
    ("[b;1]", 2, LITERAL, "("),
    ("[I;1L]", 6, "snbt.parser.invalid_array_element_type", ""),
    ("[B;-129]", 8, NUMBER_PARSE_FAILURE, ""),
    ("[I;0x100000000]", 15, NUMBER_PARSE_FAILURE, ""),
    ("12345678901", 11, LITERAL, "."),
    ("[I;12345678901]", 15, NUMBER_PARSE_FAILURE, ""),
    ("0x8000000000000000sl", 20, NUMBER_PARSE_FAILURE, ""),
    ("200sb", 5, NUMBER_PARSE_FAILURE, ""),
    (
        "[B;-0x0]",
        8,
        "snbt.parser.expected_non_negative_number",
        "",
    ),
    ("{a:1e39f}", 8, "snbt.parser.infinity_not_allowed", ""),
    (
        "\"\\UFFFFFFFF\"",
        11,
        "snbt.parser.invalid_codepoint",
        "U+FFFFFFFF",
    ),
    (
        "\"\\uD800\"",
        7,
        "snbt.parser.invalid_codepoint",
        "U+0000D800",
    ),
    (
        "\"\\uDE00\"",
        7,
        "snbt.parser.invalid_codepoint",
        "U+0000DE00",
    ),
    (
        "\"\\uD800x\"",
        7,
        "snbt.parser.invalid_codepoint",
        "U+0000D800",
    ),
    (
        "\"\\N{BACKSPACE}\"",
        14,
        "snbt.parser.invalid_character_name",
        "",
    ),
    (
        "\"\\N{SNOW MAN}\"",
        13,
        "snbt.parser.invalid_character_name",
        "",
    ),
    ("\"\\N{-A}\"", 7, "snbt.parser.invalid_character_name", ""),
    ("\"\\N{}\"", 4, "snbt.parser.invalid_character_name", ""),
    (
        "uuid(\"0-0-0-0-0\")",
        17,
        "snbt.parser.expected_string_uuid",
        "",
    ),
    ("BOOL(1)", 7, "snbt.parser.no_such_operation", "BOOL/1"),
    ("true(1)", 7, "snbt.parser.no_such_operation", "true/1"),
    ("-a", 2, LITERAL, "("),
    ("@", 0, "snbt.parser.expected_unquoted_string", ""),
];

/// The reported key, its argument and its
/// offset.
#[test]
fn a_failed_read_reports_the_first_record_at_the_furthest_position() {
    for &(input, offset, key, argument) in ERRORS {
        let error = read_tag(input, 0).expect_err(input);
        let (position, actual_key, actual_argument) = rendered(input, error);
        assert_eq!((position, actual_key), (offset, key), "{input:?}");
        if key != NUMBER_PARSE_FAILURE {
            assert_eq!(actual_argument, argument, "argument of {input:?}");
        }
        assert_eq!(validate(input), Err(error), "validation of {input:?}");
    }
}

/// The values of sections 2 and 3, in the compact form, and the text the read leaves.
#[test]
fn a_read_gives_the_tag_of_its_form_and_stops_after_it() {
    let cases: &[(&str, &str, &str)] = &[
        ("- 5", "-5", ""),
        ("1 u b", "1b", ""),
        ("1 e 5", "100000.0d", ""),
        ("\"a\\ nb\"", "\"a\\nb\"", ""),
        ("1__000", "1000", ""),
        ("{1st:2,-:3,true:4}", "{\"-\":3,\"1st\":2,\"true\":4}", ""),
        ("[B]", "[\"B\"]", ""),
        ("[B,C]", "[\"B\",\"C\"]", ""),
        ("[1,2,]", "[1,2]", ""),
        ("[1b,\"a\",{}]", "[1b,\"a\",{}]", ""),
        ("'x\"y'", "'x\"y'", ""),
        (
            "\"\\x41\\u00e9\\U0001F600\\s\\u0000\"",
            "\"Aé😀 \\x00\"",
            "",
        ),
        ("\"\\uD83D\\uDE00😀\"", "\"😀😀\"", ""),
        ("\"\\N{SNOWMAN}\\N{ snowman }\"", "\"☃☃\"", ""),
        ("\"\\N{CJK UNIFIED IDEOGRAPH-4E00}\"", "\"一\"", ""),
        ("1.", "1.0d", ""),
        (".5", "0.5d", ""),
        ("1E2", "100.0d", ""),
        ("1f", "1.0f", ""),
        ("01e1", "10.0d", ""),
        ("1e", "1", "e"),
        ("1.5b", "1.5d", "b"),
        ("1e400", "1", "e400"),
        ("-0.0", "0.0d", ""),
        ("-0.0f", "0.0f", ""),
        ("1e-400", "0.0d", ""),
        ("0b101", "5", ""),
        ("0B1L", "1L", ""),
        ("0b2", "0b", "2"),
        ("0xFFb", "4091", ""),
        ("0x10ub", "16b", ""),
        ("0x10sb", "16b", ""),
        ("1u", "1", "u"),
        ("1x", "1", "x"),
        ("TRUE", "1b", ""),
        ("-128b", "-128b", ""),
        ("-0x80sb", "-128b", ""),
        ("4294967295ui", "-1", ""),
        ("65535us", "-1s", ""),
        ("18446744073709551615ul", "-1L", ""),
        ("[I;255ub]", "[I;-1]", ""),
        ("[L;0xFFFFFFFF]", "[L;4294967295L]", ""),
        ("[L;0xFFFFFFFFi]", "[L;-1L]", ""),
        ("bool(0.5)", "1b", ""),
        ("bool(256)", "1b", ""),
        ("bool(true)", "1b", ""),
        ("bool(1,)", "1b", ""),
        (
            "uuid(\"00000001-0002-0003-0004-000000000005\")",
            "[I;1,131075,262144,5]",
            "",
        ),
        (
            "uuid(f81d4fae-7dec-11d0-a765-00a0c91e6bf6)",
            "[I;-132296786,2112623056,-1486552928,-920753162]",
            "",
        ),
        ("{a:1} x", "{a:1}", " x"),
        (r#""\b\f\r""#, r#""\b\f\r""#, ""),
        (r#""\'""#, r#""'""#, ""),
        (r#"'\"'"#, r#"'"'"#, ""),
    ];
    for &(input, compact, rest) in cases {
        let (tag, end) = read_tag(input, 0).unwrap_or_else(|e| panic!("{input:?}: {e}"));
        assert_eq!(write(&tag), compact, "{input:?}");
        assert_eq!(&input[end..], rest, "rest of {input:?}");
        assert_eq!(validate(input), Ok(end), "validation of {input:?}");
    }
}

#[test]
fn the_read_starts_at_the_cursor_and_the_compound_reads_check_the_type() {
    let line = "x {a:[1]} y";
    assert_eq!(
        read_compound(line, 2).map(|(c, end)| (write_compound(&c), end)),
        Ok(("{a:[1]}".to_owned(), 9))
    );
    assert_eq!(check_compound(line, 2), Ok(9));
    let expected = |position| SnbtError {
        kind: SnbtErrorKind::ExpectedCompound,
        position,
    };
    for (input, position) in [("5", 1), ("\"{}\"", 4)] {
        assert_eq!(read_compound(input, 0).map(drop), Err(expected(position)));
        assert_eq!(check_compound(input, 0), Err(expected(position)));
    }
    assert_eq!(parse_compound("5  "), Err(expected(1)));
    assert_eq!(
        parse_tag("{a:1} x"),
        Err(SnbtError {
            kind: SnbtErrorKind::Trailing,
            position: 6
        })
    );
}

/// 512 levels read; the `{` or `[` of level 513 stops the read.
#[test]
fn a_tag_deeper_than_512_levels_is_rejected_at_the_opening_of_level_513() {
    let lists = |levels: usize| "[".repeat(levels) + &"]".repeat(levels);
    let compounds = |levels: usize| "{a:".repeat(levels - 1) + "{}" + &"}".repeat(levels - 1);
    let alternating = |levels: usize| {
        let pairs = (levels - 1) / 2;
        let innermost = if levels.is_multiple_of(2) {
            "[{}]"
        } else {
            "[]"
        };
        "[{a:".repeat(pairs) + innermost + &"}]".repeat(pairs)
    };
    let calls = |levels: usize| "bool(".repeat(levels) + "1" + &")".repeat(levels);
    let cases: [(&dyn Fn(usize) -> String, usize); 4] = [
        (&lists, 512),
        (&compounds, 512 * 3),
        (&alternating, 256 * 4),
        (&calls, 512 * 5 + 4),
    ];
    for (nested, position) in cases {
        let (_, end) = read_tag(&nested(512), 0).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(end, nested(512).len());
        assert_eq!(validate(&nested(512)), Ok(end));
        let deep = nested(513);
        let error = read_tag(&deep, 0).unwrap_err();
        assert_eq!(
            (error.kind, error.position),
            (SnbtErrorKind::TooDeep, position),
            "{}",
            &deep[..deep.len().min(16)]
        );
        assert_eq!(validate(&deep), Err(error));
    }
    assert_eq!(
        read_tag(&calls(512), 0).map(|(tag, _)| tag),
        Ok(NbtTag::Byte(1))
    );
}

/// A string longer than the binary limit of 65535 bytes still reads.
#[test]
fn a_string_has_no_length_limit() {
    let text = "a".repeat(70_000);
    let input = format!("\"{text}\"");
    assert_eq!(read_tag(&input, 0), Ok((NbtTag::String(text), input.len())));
}

/// The match of an expected tag against an actual tag.
#[test]
fn a_tag_matches_when_the_actual_tag_holds_it() {
    let cases = [
        ("{}", "{a:1}", true),
        ("{a:1}", "{a:1,b:2}", true),
        ("{a:1b}", "{a:1}", false),
        ("{a:1,c:3}", "{a:1,b:2}", false),
        ("{a:{b:1}}", "{a:{b:1,c:2}}", true),
        ("[x,x]", "[x]", false),
        ("[x,x]", "[x,y]", true),
        ("[{id:a}]", "[{id:b,n:1},{id:a,n:2}]", true),
        ("[]", "[1]", false),
        ("[]", "[]", true),
        ("[I;1,2]", "[I;1,2,3]", false),
        ("[I;1,2]", "[I;1,2]", true),
        ("1.5f", "1.5f", true),
        ("\"a\"", "\"b\"", false),
    ];
    for (expected, actual, result) in cases {
        let (expected_tag, actual_tag) = (parse_tag(expected).unwrap(), parse_tag(actual).unwrap());
        assert_eq!(
            expected_tag.matches(&actual_tag),
            result,
            "{expected} in {actual}"
        );
    }
}
