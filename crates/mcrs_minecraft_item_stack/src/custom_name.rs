use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_text::Text;

/// The name that a player or a data pack gave to the stack.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct CustomName(pub Text);

// chisle: the hash ignores the text, so stacks that differ only in a custom name share a hash and
// cost one more equality check each. A `Hash` on `Text` lifts it.
impl Hash for CustomName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(0);
    }
}
