use mcrs_command::{LibraryError, Reader, Span, StringValue, SyntaxError};
use std::convert::Infallible;

type Result<T> = std::result::Result<T, SyntaxError<Infallible>>;

#[test]
fn integers_follow_the_number_token_grammar() {
    let cases: [(&str, Result<i32>, usize); 8] = [
        ("12 3", Ok(12), 2),
        ("-0", Ok(0), 2),
        ("007", Ok(7), 3),
        (
            "1.5",
            Err(SyntaxError::library(
                LibraryError::InvalidInt(Span::new(0, 3)),
                0,
            )),
            0,
        ),
        (
            "-",
            Err(SyntaxError::library(
                LibraryError::InvalidInt(Span::new(0, 1)),
                0,
            )),
            0,
        ),
        (
            "2147483648",
            Err(SyntaxError::library(
                LibraryError::InvalidInt(Span::new(0, 10)),
                0,
            )),
            0,
        ),
        (
            "+5",
            Err(SyntaxError::library(LibraryError::ExpectedInt, 0)),
            0,
        ),
        (
            "1-2",
            Err(SyntaxError::library(
                LibraryError::InvalidInt(Span::new(0, 3)),
                0,
            )),
            0,
        ),
    ];
    for (text, expected, cursor) in cases {
        let mut reader = Reader::new(text);
        assert_eq!(reader.read_int(), expected, "{text}");
        assert_eq!(reader.cursor(), cursor, "{text}");
    }
}

#[test]
fn doubles_follow_the_decimal_grammar_and_stay_finite() {
    let long_one = format!("1{}", "0".repeat(400));
    let cases: [(&str, Result<f64>, usize); 8] = [
        (".5", Ok(0.5), 2),
        ("5.", Ok(5.0), 2),
        ("-.5", Ok(-0.5), 3),
        (
            ".",
            Err(SyntaxError::library(
                LibraryError::InvalidDouble(Span::new(0, 1)),
                0,
            )),
            0,
        ),
        (
            "1.2.3",
            Err(SyntaxError::library(
                LibraryError::InvalidDouble(Span::new(0, 5)),
                0,
            )),
            0,
        ),
        ("1e5", Ok(1.0), 1),
        (
            "-",
            Err(SyntaxError::library(
                LibraryError::InvalidDouble(Span::new(0, 1)),
                0,
            )),
            0,
        ),
        (
            &long_one,
            Err(SyntaxError::library(
                LibraryError::InvalidDouble(Span::new(0, 401)),
                0,
            )),
            0,
        ),
    ];
    for (text, expected, cursor) in cases {
        let mut reader = Reader::new(text);
        assert_eq!(reader.read_double(), expected, "{text}");
        assert_eq!(reader.cursor(), cursor, "{text}");
    }
    let negative_zero: f64 = Reader::new("-0").read_double::<Infallible>().unwrap();
    assert!(negative_zero.is_sign_negative());
}

#[test]
fn longs_follow_the_number_token_grammar_with_the_64_bit_range() {
    let cases: [(&str, Result<i64>, usize); 4] = [
        ("9223372036854775807", Ok(i64::MAX), 19),
        ("-9223372036854775808 ", Ok(i64::MIN), 20),
        (
            "9223372036854775808",
            Err(SyntaxError::library(
                LibraryError::InvalidLong(Span::new(0, 19)),
                0,
            )),
            0,
        ),
        (
            "x",
            Err(SyntaxError::library(LibraryError::ExpectedLong, 0)),
            0,
        ),
    ];
    for (text, expected, cursor) in cases {
        let mut reader = Reader::new(text);
        assert_eq!(reader.read_long(), expected, "{text}");
        assert_eq!(reader.cursor(), cursor, "{text}");
    }
}

#[test]
fn a_float_rounds_once_from_the_decimal_text_and_stays_finite() {
    let mut reader = Reader::new("1.00000017881393432617187499");
    assert_eq!(reader.read_float::<Infallible>(), Ok(1.000_000_1_f32));

    let above_max = format!("1{}", "0".repeat(39));
    let cases: [(&str, Result<f32>, usize); 3] = [
        (
            &above_max,
            Err(SyntaxError::library(
                LibraryError::InvalidFloat(Span::new(0, 40)),
                0,
            )),
            0,
        ),
        (
            "1.2.3",
            Err(SyntaxError::library(
                LibraryError::InvalidFloat(Span::new(0, 5)),
                0,
            )),
            0,
        ),
        (
            "e",
            Err(SyntaxError::library(LibraryError::ExpectedFloat, 0)),
            0,
        ),
    ];
    for (text, expected, cursor) in cases {
        let mut reader = Reader::new(text);
        assert_eq!(reader.read_float(), expected, "{text}");
        assert_eq!(reader.cursor(), cursor, "{text}");
    }
}

#[test]
fn an_expected_symbol_is_skipped_or_fails_at_the_cursor() {
    let cases: [(&str, usize, Result<()>, usize); 3] = [
        ("a[", 1, Ok(()), 2),
        (
            "a]",
            1,
            Err(SyntaxError::library(LibraryError::ExpectedSymbol('['), 1)),
            1,
        ),
        (
            "a",
            1,
            Err(SyntaxError::library(LibraryError::ExpectedSymbol('['), 1)),
            1,
        ),
    ];
    for (text, start, expected, cursor) in cases {
        let mut reader = Reader::at(text, start);
        assert_eq!(reader.expect('['), expected, "{text}");
        assert_eq!(reader.cursor(), cursor, "{text}");
    }
}

#[test]
fn booleans_are_read_as_strings() {
    let cases: [(&str, Result<bool>); 3] = [
        ("\"true\"", Ok(true)),
        (
            "True",
            Err(SyntaxError::library(
                LibraryError::InvalidBool(mcrs_command::StringSpan {
                    span: Span::new(0, 4),
                    quoted: false,
                }),
                0,
            )),
        ),
        (
            "\"\"",
            Err(SyntaxError::library(LibraryError::ExpectedBool, 2)),
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(Reader::new(text).read_bool(), expected, "{text}");
    }
}

#[test]
fn a_quoted_string_unescapes_only_its_terminator_and_the_backslash() {
    let input = r#""a\"b\\c'd" rest"#;
    let mut reader = Reader::new(input);
    let string = reader.read_string::<Infallible>().unwrap();
    assert!(StringValue::new(input, string).eq_str(r#"a"b\c'd"#));
    assert_eq!(reader.remaining(), " rest");

    let cases: [(&str, LibraryError, usize); 3] = [
        (r#"'a\"'"#, LibraryError::InvalidEscape('"'), 3),
        ("\"open", LibraryError::ExpectedEndOfQuote, 5),
        ("\"ends\\", LibraryError::ExpectedEndOfQuote, 6),
    ];
    for (text, kind, position) in cases {
        assert_eq!(
            Reader::new(text).read_string::<Infallible>(),
            Err(SyntaxError::library(kind, position)),
            "{text}"
        );
    }
}
