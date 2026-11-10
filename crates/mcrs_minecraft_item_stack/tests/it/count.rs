use mcrs_minecraft_item_stack::Count;

#[test]
fn count_holds_one_to_255_and_rejects_zero() {
    let cases: [(u8, Option<u8>); 4] = [(0, None), (1, Some(1)), (99, Some(99)), (255, Some(255))];
    for (input, expected) in cases {
        assert_eq!(Count::new(input).map(Count::get), expected, "count {input}");
    }
}
