use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_text::Text;

/// The name of the item, which a custom name replaces.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct ItemName(pub Text);

// chisle: the hash of each text-bearing component ignores the text, so stacks that differ only
// in a text share a hash and cost one more equality check each. A `Hash` on `Text` lifts it.
impl Hash for ItemName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(0);
    }
}
