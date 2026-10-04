use mcrs_minecraft_nbt::tag::NbtTag;

pub fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// Vanilla writes compounds in hash order, so trees are compared with every
/// compound sorted by key.
pub fn sorted(tag: NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(mut compound) => {
            compound.child_tags = compound
                .child_tags
                .into_iter()
                .map(|(key, value)| (key, sorted(value)))
                .collect();
            compound.child_tags.sort_by(|a, b| a.0.cmp(&b.0));
            NbtTag::Compound(compound)
        }
        NbtTag::List(list) => NbtTag::List(list.into_iter().map(sorted).collect()),
        other => other,
    }
}

pub fn nbt_tree(bytes: &[u8]) -> NbtTag {
    sorted(mcrs_minecraft_nbt::from_bytes_unnamed(&mut std::io::Cursor::new(bytes)).unwrap())
}
