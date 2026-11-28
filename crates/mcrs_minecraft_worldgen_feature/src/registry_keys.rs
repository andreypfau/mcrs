use mcrs_minecraft_resource::{Registered, RegistryKey, ResourceLocation, rl};

pub const POS_RULE_TEST: RegistryKey<crate::pos_rule_test_keys::PosRuleTestType> =
    <crate::pos_rule_test_keys::PosRuleTestType as Registered>::REGISTRY;
impl Registered for crate::pos_rule_test_keys::PosRuleTestType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:pos_rule_test"));
}

pub const RULE_BLOCK_ENTITY_MODIFIER: RegistryKey<
    crate::rule_block_entity_modifier_keys::RuleBlockEntityModifierType,
> = <crate::rule_block_entity_modifier_keys::RuleBlockEntityModifierType as Registered>::REGISTRY;
impl Registered for crate::rule_block_entity_modifier_keys::RuleBlockEntityModifierType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:rule_block_entity_modifier"));
}

pub const RULE_TEST_TYPE: RegistryKey<crate::rule_test_type_keys::RuleTestType> =
    <crate::rule_test_type_keys::RuleTestType as Registered>::REGISTRY;
impl Registered for crate::rule_test_type_keys::RuleTestType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:rule_test_type"));
}

pub const FEATURE: RegistryKey<crate::proto::Feature> =
    <crate::proto::Feature as Registered>::REGISTRY;
impl Registered for crate::proto::Feature {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/feature"));
}

pub const FEATURE_SIZE_TYPE: RegistryKey<crate::feature_size_type_keys::FeatureSizeType> =
    <crate::feature_size_type_keys::FeatureSizeType as Registered>::REGISTRY;
impl Registered for crate::feature_size_type_keys::FeatureSizeType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/feature_size_type"));
}

pub const FEATURE_TYPE: RegistryKey<crate::feature_type_keys::FeatureType> =
    <crate::feature_type_keys::FeatureType as Registered>::REGISTRY;
impl Registered for crate::feature_type_keys::FeatureType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/feature_type"));
}

pub const FOLIAGE_PLACER_TYPE: RegistryKey<crate::foliage_placer_type_keys::FoliagePlacerType> =
    <crate::foliage_placer_type_keys::FoliagePlacerType as Registered>::REGISTRY;
impl Registered for crate::foliage_placer_type_keys::FoliagePlacerType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/foliage_placer_type"));
}

pub const PLACEMENT_MODIFIER_TYPE: RegistryKey<
    crate::placement_modifier_type_keys::PlacementModifierType,
> = <crate::placement_modifier_type_keys::PlacementModifierType as Registered>::REGISTRY;
impl Registered for crate::placement_modifier_type_keys::PlacementModifierType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/placement_modifier_type"));
}

pub const PROCESSOR_LIST: RegistryKey<crate::proto::StructureProcessorList> =
    <crate::proto::StructureProcessorList as Registered>::REGISTRY;
impl Registered for crate::proto::StructureProcessorList {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/processor_list"));
}

pub const ROOT_PLACER_TYPE: RegistryKey<crate::root_placer_type_keys::RootPlacerType> =
    <crate::root_placer_type_keys::RootPlacerType as Registered>::REGISTRY;
impl Registered for crate::root_placer_type_keys::RootPlacerType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/root_placer_type"));
}

pub const STRUCTURE_POOL_ELEMENT: RegistryKey<
    crate::structure_pool_element_keys::StructurePoolElementType,
> = <crate::structure_pool_element_keys::StructurePoolElementType as Registered>::REGISTRY;
impl Registered for crate::structure_pool_element_keys::StructurePoolElementType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/structure_pool_element"));
}

pub const STRUCTURE_PROCESSOR: RegistryKey<
    crate::structure_processor_keys::StructureProcessorType,
> = <crate::structure_processor_keys::StructureProcessorType as Registered>::REGISTRY;
impl Registered for crate::structure_processor_keys::StructureProcessorType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/structure_processor"));
}

pub const TEMPLATE_POOL: RegistryKey<crate::pool::TemplatePool> =
    <crate::pool::TemplatePool as Registered>::REGISTRY;
impl Registered for crate::pool::TemplatePool {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/template_pool"));
}

pub const TREE_DECORATOR_TYPE: RegistryKey<crate::tree_decorator_type_keys::TreeDecoratorType> =
    <crate::tree_decorator_type_keys::TreeDecoratorType as Registered>::REGISTRY;
impl Registered for crate::tree_decorator_type_keys::TreeDecoratorType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/tree_decorator_type"));
}

pub const TRUNK_PLACER_TYPE: RegistryKey<crate::trunk_placer_type_keys::TrunkPlacerType> =
    <crate::trunk_placer_type_keys::TrunkPlacerType as Registered>::REGISTRY;
impl Registered for crate::trunk_placer_type_keys::TrunkPlacerType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:worldgen/trunk_placer_type"));
}

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
