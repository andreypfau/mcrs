use super::data::EnchantmentData;
use mcrs_minecraft_core::tag_key::TaggedRegistry;

impl TaggedRegistry for EnchantmentData {
    const REGISTRY_PATH: &'static str = "enchantment";
}
