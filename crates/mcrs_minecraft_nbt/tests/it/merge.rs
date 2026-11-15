use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

fn compound(entries: Vec<(&str, NbtTag)>) -> NbtCompound {
    let mut compound = NbtCompound::new();
    for (key, value) in entries {
        compound.put(key, value);
    }
    compound
}

#[test]
fn loot_mod_049_compound_merge_merges_compounds_and_copies_other_values() {
    let mut target = compound(vec![
        (
            "a",
            compound(vec![("x", NbtTag::Int(1)), ("y", NbtTag::Int(2))]).into(),
        ),
        ("b", NbtTag::Int(1)),
        ("c", NbtTag::Int(5)),
        ("f", compound(vec![("k", NbtTag::Int(1))]).into()),
    ]);
    let source = compound(vec![
        (
            "a",
            compound(vec![("y", NbtTag::Int(3)), ("z", NbtTag::Int(4))]).into(),
        ),
        (
            "b",
            compound(vec![("n", compound(vec![("m", NbtTag::Int(1))]).into())]).into(),
        ),
        ("d", NbtTag::List(vec![NbtTag::Int(1)])),
        ("f", NbtTag::Int(2)),
        (
            "e",
            compound(vec![("p", compound(vec![("q", NbtTag::Int(1))]).into())]).into(),
        ),
    ]);
    target.merge(&source);
    let expected = compound(vec![
        (
            "a",
            compound(vec![
                ("x", NbtTag::Int(1)),
                ("y", NbtTag::Int(3)),
                ("z", NbtTag::Int(4)),
            ])
            .into(),
        ),
        (
            "b",
            compound(vec![("n", compound(vec![("m", NbtTag::Int(1))]).into())]).into(),
        ),
        ("c", NbtTag::Int(5)),
        ("f", NbtTag::Int(2)),
        ("d", NbtTag::List(vec![NbtTag::Int(1)])),
        (
            "e",
            compound(vec![("p", compound(vec![("q", NbtTag::Int(1))]).into())]).into(),
        ),
    ]);
    assert_eq!(target, expected);
}
