use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct CustomData(pub NbtCompound);

impl PartialEq for CustomData {
    fn eq(&self, other: &Self) -> bool {
        compounds_equal(&self.0, &other.0)
    }
}

impl Eq for CustomData {}

impl Hash for CustomData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.child_tags.len().hash(state);
    }
}

pub(crate) fn compounds_equal(left: &NbtCompound, right: &NbtCompound) -> bool {
    left.child_tags.len() == right.child_tags.len()
        && contains_each_entry(left, right)
        && contains_each_entry(right, left)
}

fn contains_each_entry(entries: &NbtCompound, within: &NbtCompound) -> bool {
    entries.child_tags.iter().all(|(key, value)| {
        within
            .child_tags
            .iter()
            .any(|(other_key, other)| key == other_key && tags_equal(value, other))
    })
}

fn tags_equal(left: &NbtTag, right: &NbtTag) -> bool {
    match (left, right) {
        (NbtTag::Compound(left), NbtTag::Compound(right)) => compounds_equal(left, right),
        (NbtTag::List(left), NbtTag::List(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| tags_equal(left, right))
        }
        (left, right) => left == right,
    }
}
