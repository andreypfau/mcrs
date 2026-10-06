use mcrs_minecraft_loot::IntExpression;

/// An int constant reads as `Codec.INT` does: a JSON number keeps the low bits
/// of its integer part.
#[test]
fn an_int_constant_reads_any_number_as_java_does() {
    let cases = [
        ("5", 5),
        ("2.9", 2),
        ("3000000000", -1_294_967_296),
        ("1e10", 1_410_065_408),
        (r#"{"type":"minecraft:constant","value":7}"#, 7),
    ];
    for (json, expected) in cases {
        let read: IntExpression =
            serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
        assert_eq!(read, IntExpression::Constant(expected), "{json}");
    }
}
