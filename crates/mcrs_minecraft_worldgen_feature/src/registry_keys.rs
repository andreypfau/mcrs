// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, ResourceLocation, rl};

pub const POS_RULE_TEST: RegistryKey<crate::pos_rule_test_keys::PosRuleTestType> =
    RegistryKey::new(rl!("minecraft:pos_rule_test"));
impl Registered for crate::pos_rule_test_keys::PosRuleTestType {
    const REGISTRY: RegistryKey<Self> = POS_RULE_TEST;
}

pub const RULE_BLOCK_ENTITY_MODIFIER: RegistryKey<
    crate::rule_block_entity_modifier_keys::RuleBlockEntityModifierType,
> = RegistryKey::new(rl!("minecraft:rule_block_entity_modifier"));
impl Registered for crate::rule_block_entity_modifier_keys::RuleBlockEntityModifierType {
    const REGISTRY: RegistryKey<Self> = RULE_BLOCK_ENTITY_MODIFIER;
}

pub const RULE_TEST_TYPE: RegistryKey<crate::rule_test_type_keys::RuleTestType> =
    RegistryKey::new(rl!("minecraft:rule_test_type"));
impl Registered for crate::rule_test_type_keys::RuleTestType {
    const REGISTRY: RegistryKey<Self> = RULE_TEST_TYPE;
}

pub const FEATURE: RegistryKey<crate::proto::Feature> =
    RegistryKey::new(rl!("minecraft:worldgen/feature"));
impl Registered for crate::proto::Feature {
    const REGISTRY: RegistryKey<Self> = FEATURE;
}

pub const FEATURE_SIZE_TYPE: RegistryKey<crate::feature_size_type_keys::FeatureSizeType> =
    RegistryKey::new(rl!("minecraft:worldgen/feature_size_type"));
impl Registered for crate::feature_size_type_keys::FeatureSizeType {
    const REGISTRY: RegistryKey<Self> = FEATURE_SIZE_TYPE;
}

pub const FEATURE_TYPE: RegistryKey<crate::feature_type_keys::FeatureType> =
    RegistryKey::new(rl!("minecraft:worldgen/feature_type"));
impl Registered for crate::feature_type_keys::FeatureType {
    const REGISTRY: RegistryKey<Self> = FEATURE_TYPE;
}

pub const FOLIAGE_PLACER_TYPE: RegistryKey<crate::foliage_placer_type_keys::FoliagePlacerType> =
    RegistryKey::new(rl!("minecraft:worldgen/foliage_placer_type"));
impl Registered for crate::foliage_placer_type_keys::FoliagePlacerType {
    const REGISTRY: RegistryKey<Self> = FOLIAGE_PLACER_TYPE;
}

pub const PLACEMENT_MODIFIER_TYPE: RegistryKey<
    crate::placement_modifier_type_keys::PlacementModifierType,
> = RegistryKey::new(rl!("minecraft:worldgen/placement_modifier_type"));
impl Registered for crate::placement_modifier_type_keys::PlacementModifierType {
    const REGISTRY: RegistryKey<Self> = PLACEMENT_MODIFIER_TYPE;
}

pub const PROCESSOR_LIST: RegistryKey<crate::proto::StructureProcessorList> =
    RegistryKey::new(rl!("minecraft:worldgen/processor_list"));
impl Registered for crate::proto::StructureProcessorList {
    const REGISTRY: RegistryKey<Self> = PROCESSOR_LIST;
}

pub const ROOT_PLACER_TYPE: RegistryKey<crate::root_placer_type_keys::RootPlacerType> =
    RegistryKey::new(rl!("minecraft:worldgen/root_placer_type"));
impl Registered for crate::root_placer_type_keys::RootPlacerType {
    const REGISTRY: RegistryKey<Self> = ROOT_PLACER_TYPE;
}

pub const STRUCTURE_POOL_ELEMENT: RegistryKey<
    crate::structure_pool_element_keys::StructurePoolElementType,
> = RegistryKey::new(rl!("minecraft:worldgen/structure_pool_element"));
impl Registered for crate::structure_pool_element_keys::StructurePoolElementType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_POOL_ELEMENT;
}

pub const STRUCTURE_PROCESSOR: RegistryKey<
    crate::structure_processor_keys::StructureProcessorType,
> = RegistryKey::new(rl!("minecraft:worldgen/structure_processor"));
impl Registered for crate::structure_processor_keys::StructureProcessorType {
    const REGISTRY: RegistryKey<Self> = STRUCTURE_PROCESSOR;
}

pub const TEMPLATE_POOL: RegistryKey<crate::pool::TemplatePool> =
    RegistryKey::new(rl!("minecraft:worldgen/template_pool"));
impl Registered for crate::pool::TemplatePool {
    const REGISTRY: RegistryKey<Self> = TEMPLATE_POOL;
}

pub const TREE_DECORATOR_TYPE: RegistryKey<crate::tree_decorator_type_keys::TreeDecoratorType> =
    RegistryKey::new(rl!("minecraft:worldgen/tree_decorator_type"));
impl Registered for crate::tree_decorator_type_keys::TreeDecoratorType {
    const REGISTRY: RegistryKey<Self> = TREE_DECORATOR_TYPE;
}

pub const TRUNK_PLACER_TYPE: RegistryKey<crate::trunk_placer_type_keys::TrunkPlacerType> =
    RegistryKey::new(rl!("minecraft:worldgen/trunk_placer_type"));
impl Registered for crate::trunk_placer_type_keys::TrunkPlacerType {
    const REGISTRY: RegistryKey<Self> = TRUNK_PLACER_TYPE;
}

/// The static registries of this crate, which the catalog does not list.
pub const STATIC_REGISTRIES: &[(ResourceLocation<'static>, &[ResourceLocation<'static>])] = &[
    (
        RULE_TEST_TYPE.location(),
        crate::rule_test_type_keys::RuleTestType::ENTRIES,
    ),
    (
        RULE_BLOCK_ENTITY_MODIFIER.location(),
        crate::rule_block_entity_modifier_keys::RuleBlockEntityModifierType::ENTRIES,
    ),
    (
        POS_RULE_TEST.location(),
        crate::pos_rule_test_keys::PosRuleTestType::ENTRIES,
    ),
    (
        FEATURE_TYPE.location(),
        crate::feature_type_keys::FeatureType::ENTRIES,
    ),
    (
        PLACEMENT_MODIFIER_TYPE.location(),
        crate::placement_modifier_type_keys::PlacementModifierType::ENTRIES,
    ),
    (
        FOLIAGE_PLACER_TYPE.location(),
        crate::foliage_placer_type_keys::FoliagePlacerType::ENTRIES,
    ),
    (
        TRUNK_PLACER_TYPE.location(),
        crate::trunk_placer_type_keys::TrunkPlacerType::ENTRIES,
    ),
    (
        ROOT_PLACER_TYPE.location(),
        crate::root_placer_type_keys::RootPlacerType::ENTRIES,
    ),
    (
        TREE_DECORATOR_TYPE.location(),
        crate::tree_decorator_type_keys::TreeDecoratorType::ENTRIES,
    ),
    (
        FEATURE_SIZE_TYPE.location(),
        crate::feature_size_type_keys::FeatureSizeType::ENTRIES,
    ),
    (
        STRUCTURE_PROCESSOR.location(),
        crate::structure_processor_keys::StructureProcessorType::ENTRIES,
    ),
    (
        STRUCTURE_POOL_ELEMENT.location(),
        crate::structure_pool_element_keys::StructurePoolElementType::ENTRIES,
    ),
];
