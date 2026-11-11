use core::hash::Hash;
use core::hash::Hasher;
use core::mem;

use mcrs_minecraft_text::Text;

/// How the tooltip shows an attribute modifier.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ModifierDisplay {
    #[default]
    Default,
    Hidden,
    /// The tooltip shows this text in place of the modifier.
    Override(Text),
}

// chisle: the hash ignores the text of an override, so displays that differ only in that text
// share a hash and cost one more equality check each. A `Hash` on `Text` lifts it.
impl Hash for ModifierDisplay {
    fn hash<H: Hasher>(&self, state: &mut H) {
        mem::discriminant(self).hash(state);
    }
}
