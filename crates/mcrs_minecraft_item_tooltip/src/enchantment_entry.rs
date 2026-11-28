use mcrs_minecraft_text::Text;

use crate::EnchantmentClass;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnchantmentEntry<'a> {
    pub description: &'a Text,
    pub max_level: u8,
    pub class: EnchantmentClass,
}
