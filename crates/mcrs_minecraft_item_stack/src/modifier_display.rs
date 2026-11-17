use core::hash::Hash;
use core::hash::Hasher;
use core::mem;

use mcrs_minecraft_text::Text;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ModifierDisplay {
    #[default]
    Default,
    Hidden,
    Override(Text),
}

impl Hash for ModifierDisplay {
    fn hash<H: Hasher>(&self, state: &mut H) {
        mem::discriminant(self).hash(state);
    }
}
