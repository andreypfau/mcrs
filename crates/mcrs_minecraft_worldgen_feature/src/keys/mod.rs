// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod feature;
pub mod feature_size_type;
pub mod feature_tags;
pub mod feature_type;
pub mod foliage_placer_type;
pub mod placed_feature;
pub mod placement_modifier_type;
pub mod pos_rule_test;
pub mod processor_list;
pub mod root_placer_type;
pub mod rule_block_entity_modifier;
pub mod rule_test_type;
pub mod structure_pool_element;
pub mod structure_processor;
pub mod template_pool;
pub mod tree_decorator_type;
pub mod trunk_placer_type;

pub use pos_rule_test::PosRuleTestType;
pub use rule_block_entity_modifier::RuleBlockEntityModifierType;
pub use rule_test_type::RuleTestType;
pub use feature_size_type::FeatureSizeType;
pub use feature_type::FeatureType;
pub use foliage_placer_type::FoliagePlacerType;
pub use placement_modifier_type::PlacementModifierType;
pub use root_placer_type::RootPlacerType;
pub use structure_pool_element::StructurePoolElementType;
pub use structure_processor::StructureProcessorType;
pub use tree_decorator_type::TreeDecoratorType;
pub use trunk_placer_type::TrunkPlacerType;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const POS_RULE_TEST: RegistryKey<crate::keys::PosRuleTestType> = RegistryKey::new(rl!("minecraft:pos_rule_test"));
impl Registered for crate::keys::PosRuleTestType {
    const REGISTRY: RegistryKey<Self> = POS_RULE_TEST;
}

pub const RULE_BLOCK_ENTITY_MODIFIER: RegistryKey<crate::keys::RuleBlockEntityModifierType> = RegistryKey::new(rl!("minecraft:rule_block_entity_modifier"));
impl Registered for crate::keys::RuleBlockEntityModifierType {
    const REGISTRY: RegistryKey<Self> = RULE_BLOCK_ENTITY_MODIFIER;
}

pub const RULE_TEST_TYPE: RegistryKey<crate::keys::RuleTestType> = RegistryKey::new(rl!("minecraft:rule_test_type"));
impl Registered for crate::keys::RuleTestType {
    const REGISTRY: RegistryKey<Self> = RULE_TEST_TYPE;
}

pub const FEATURE: RegistryKey<crate::proto::Feature> = RegistryKey::new(rl!("minecraft:worldgen/feature"));
impl Registered for crate::proto::Feature {
    const REGISTRY: RegistryKey<Self> = FEATURE;
}

pub const FEATURE_SIZE_TYPE: RegistryKey<crate::keys::FeatureSizeType> = RegistryKey::new(rl!("minecraft:worldgen/feature_size_type"));
impl Registered for crate::keys::FeatureSizeType {
    const REGISTRY: RegistryKey<Self> = FEATURE_SIZE_TYPE;
}

pub const FEATURE_TYPE: RegistryKey<crate::keys::FeatureType> = RegistryKey::new(rl!("minecraft:worldgen/feature_type"));
impl Registered for crate::keys::FeatureType {
    const REGISTRY: RegistryKey<Self> = FEATURE_TYPE;
}

pub const FOLIAGE_PLACER_TYPE: RegistryKey<crate::keys::FoliagePlacerType> = RegistryKey::new(rl!("minecraft:worldgen/foliage_placer_type"));
impl Registered for crate::keys::FoliagePlacerType {
    const REGISTRY: RegistryKey<Self> = FOLIAGE_PLACER_TYPE;
}

pub const PLACED_FEATURE: RegistryKey<crate::proto::PlacedFeature> = RegistryKey::new(rl!("minecraft:worldgen/placed_feature"));
impl Registered for crate::proto::PlacedFeature {
    const REGISTRY: RegistryKey<Self> = PLACED_FEATURE;
}

pub const PLACEMENT_MODIFIER_TYPE: RegistryKey<crate::keys::PlacementModifierType> = RegistryKey::new(rl!("minecraft:worldgen/placement_modifier_type"));
impl Registered for crate::keys::PlacementModifierType {
    const REGISTRY: RegistryKey<Self> = PLACEMENT_MODIFIER_TYPE;
}

pub const PROCESSOR_LIST: RegistryKey<crate::proto::StructureProcessorList> = RegistryKey::new(rl!("minecraft:worldgen/processor_list"));
impl Registered for crate::proto::StructureProcessorList {
    const REGISTRY: RegistryKey<Self> = PROCESSOR_LIST;
}

pub const ROOT_PLACER_TYPE: RegistryKey<crate::keys::RootPlacerType> = RegistryKey::new(rl!("minecraft:worldgen/root_placer_type"));
impl Registered for crate::keys::RootPlacerType {
    const REGISTRY: RegistryKey<Self> = ROOT_PLACER_TYPE;
}

pub const STRUCTURE_POOL_ELEMENT: RegistryKey<crate::keys::StructurePoolElementType> = RegistryKey::new(rl!("minecraft:worldgen/structure_pool_element"));
impl Registered for crate::keys::StructurePoolElementType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_POOL_ELEMENT;
}

pub const STRUCTURE_PROCESSOR: RegistryKey<crate::keys::StructureProcessorType> = RegistryKey::new(rl!("minecraft:worldgen/structure_processor"));
impl Registered for crate::keys::StructureProcessorType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_PROCESSOR;
}

pub const TEMPLATE_POOL: RegistryKey<crate::pool::TemplatePool> = RegistryKey::new(rl!("minecraft:worldgen/template_pool"));
impl Registered for crate::pool::TemplatePool {
    const REGISTRY: RegistryKey<Self> = TEMPLATE_POOL;
}

pub const TREE_DECORATOR_TYPE: RegistryKey<crate::keys::TreeDecoratorType> = RegistryKey::new(rl!("minecraft:worldgen/tree_decorator_type"));
impl Registered for crate::keys::TreeDecoratorType {
    const REGISTRY: RegistryKey<Self> = TREE_DECORATOR_TYPE;
}

pub const TRUNK_PLACER_TYPE: RegistryKey<crate::keys::TrunkPlacerType> = RegistryKey::new(rl!("minecraft:worldgen/trunk_placer_type"));
impl Registered for crate::keys::TrunkPlacerType {
    const REGISTRY: RegistryKey<Self> = TRUNK_PLACER_TYPE;
}

pub fn bindings() -> [TypeBinding; 16] {
    [
        POS_RULE_TEST.binding(),
        RULE_BLOCK_ENTITY_MODIFIER.binding(),
        RULE_TEST_TYPE.binding(),
        FEATURE.binding(),
        FEATURE_SIZE_TYPE.binding(),
        FEATURE_TYPE.binding(),
        FOLIAGE_PLACER_TYPE.binding(),
        PLACED_FEATURE.binding(),
        PLACEMENT_MODIFIER_TYPE.binding(),
        PROCESSOR_LIST.binding(),
        ROOT_PLACER_TYPE.binding(),
        STRUCTURE_POOL_ELEMENT.binding(),
        STRUCTURE_PROCESSOR.binding(),
        TEMPLATE_POOL.binding(),
        TREE_DECORATOR_TYPE.binding(),
        TRUNK_PLACER_TYPE.binding(),
    ]
}
