#![expect(clippy::panic, reason = "a test helper reports a failure by panicking")]

mod grammar;
mod operation;
mod select;

use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt_path::{NbtPath, SelectedTag};

fn tag(snbt: &str) -> NbtTag {
    mcrs_minecraft_snbt::parse_tag(snbt).unwrap_or_else(|error| panic!("{snbt}: {error}"))
}

fn list(snbt: &str) -> Vec<NbtTag> {
    match tag(snbt) {
        NbtTag::List(list) => list,
        other => panic!("{snbt} is not a list: {other:?}"),
    }
}

fn path(text: &str) -> NbtPath {
    text.parse()
        .unwrap_or_else(|error| panic!("{text}: {error}"))
}

fn selected(text: &str, root: &NbtTag) -> Vec<NbtTag> {
    let mut selected = Vec::new();
    path(text).select(root, &mut selected);
    selected.into_iter().map(SelectedTag::into_owned).collect()
}
