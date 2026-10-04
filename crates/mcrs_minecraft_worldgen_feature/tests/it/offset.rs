use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{from_tag, to_nbt_tag};
use mcrs_minecraft_worldgen_feature::block_predicate::Offset;

#[test]
fn an_offset_is_an_int_array_in_nbt_and_a_list_in_json() {
    let offset = Offset([1, -2, 3]);

    assert_eq!(
        to_nbt_tag(&offset).unwrap(),
        NbtTag::IntArray(vec![1, -2, 3])
    );
    assert_eq!(serde_json::to_string(&offset).unwrap(), "[1,-2,3]");

    assert_eq!(
        from_tag::<Offset>(NbtTag::IntArray(vec![1, -2, 3])).unwrap(),
        offset
    );
    assert_eq!(serde_json::from_str::<Offset>("[1,-2,3]").unwrap(), offset);
}
