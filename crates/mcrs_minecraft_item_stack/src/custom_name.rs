use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_text::Text;

/// The name that a player or a data pack gave to the stack.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct CustomName(pub Text);

impl Hash for CustomName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(0);
    }
}
