use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_registry::StaticRegistry;

pub fn test_corpus() -> &'static (Blocks, Items) {
    mcrs_minecraft_item::test_corpus()
}

pub fn test_enchantments() -> &'static StaticRegistry<EnchantmentData> {
    mcrs_minecraft_item::enchantment::test_enchantments()
}
