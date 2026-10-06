use crate::keys::{Owner, ValueType};

pub const OWNERS: &[Owner] = &[
    Owner {
        registry: "minecraft:timeline",
        krate: "mcrs_minecraft_environment",
        value: ValueType::Defined("crate::timeline::Timeline"),
    },
    Owner {
        registry: "minecraft:world_clock",
        krate: "mcrs_minecraft_environment",
        value: ValueType::Defined("crate::world_clock::WorldClock"),
    },
    Owner {
        registry: "minecraft:banner_pattern",
        krate: "mcrs_minecraft_item",
        value: ValueType::Defined("crate::BannerPattern"),
    },
    Owner {
        registry: "minecraft:instrument",
        krate: "mcrs_minecraft_item",
        value: ValueType::Defined("crate::InstrumentValue"),
    },
    Owner {
        registry: "minecraft:jukebox_song",
        krate: "mcrs_minecraft_item",
        value: ValueType::Defined("crate::JukeboxSong"),
    },
    Owner {
        registry: "minecraft:painting_variant",
        krate: "mcrs_minecraft_item",
        value: ValueType::Defined("crate::PaintingVariantValue"),
    },
    Owner {
        registry: "minecraft:trim_material",
        krate: "mcrs_minecraft_item",
        value: ValueType::Defined("crate::TrimMaterial"),
    },
    Owner {
        registry: "minecraft:trim_pattern",
        krate: "mcrs_minecraft_item",
        value: ValueType::Defined("crate::TrimPattern"),
    },
    Owner {
        registry: "minecraft:sound_event",
        krate: "mcrs_minecraft_sound",
        value: ValueType::Defined("crate::SoundEvent"),
    },
    Owner {
        registry: "minecraft:damage_type",
        krate: "mcrs_minecraft_item",
        value: ValueType::Defined("crate::damage_type::DamageType"),
    },
    Owner {
        registry: "minecraft:decorated_pot_pattern",
        krate: "mcrs_minecraft_item",
        value: ValueType::Defined("crate::decorated_pot_pattern::DecoratedPotPattern"),
    },
    Owner {
        registry: "minecraft:dialog",
        krate: "mcrs_minecraft_item",
        value: ValueType::Defined("crate::dialog::Dialog"),
    },
    Owner {
        registry: "minecraft:block_transformer",
        krate: "mcrs_minecraft_item",
        value: ValueType::Defined("crate::block_transformer::BlockTransformer"),
    },
    Owner {
        registry: "minecraft:enchantment",
        krate: "mcrs_minecraft_item",
        value: ValueType::Defined("crate::enchantment::EnchantmentData"),
    },
    Owner {
        registry: "minecraft:wolf_variant",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Defined("crate::variant::WolfVariant"),
    },
    Owner {
        registry: "minecraft:pig_variant",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Defined("crate::variant::PigVariant"),
    },
    Owner {
        registry: "minecraft:cow_variant",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Defined("crate::variant::CowVariant"),
    },
    Owner {
        registry: "minecraft:chicken_variant",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Defined("crate::variant::ChickenVariant"),
    },
    Owner {
        registry: "minecraft:cat_variant",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Defined("crate::variant::CatVariant"),
    },
    Owner {
        registry: "minecraft:frog_variant",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Defined("crate::variant::FrogVariant"),
    },
    Owner {
        registry: "minecraft:zombie_nautilus_variant",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Defined("crate::variant::ZombieNautilusVariant"),
    },
    Owner {
        registry: "minecraft:wolf_sound_variant",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Defined("crate::variant::WolfSoundVariant"),
    },
    Owner {
        registry: "minecraft:pig_sound_variant",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Defined("crate::variant::PigSoundVariant"),
    },
    Owner {
        registry: "minecraft:cow_sound_variant",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Defined("crate::variant::CowSoundVariant"),
    },
    Owner {
        registry: "minecraft:chicken_sound_variant",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Defined("crate::variant::ChickenSoundVariant"),
    },
    Owner {
        registry: "minecraft:cat_sound_variant",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Defined("crate::variant::CatSoundVariant"),
    },
    Owner {
        registry: "minecraft:chat_type",
        krate: "mcrs_minecraft_world",
        value: ValueType::Defined("crate::chat_type::ChatType"),
    },
    Owner {
        registry: "minecraft:test_environment",
        krate: "mcrs_minecraft_world",
        value: ValueType::Defined("crate::test_types::TestEnvironment"),
    },
    Owner {
        registry: "minecraft:test_instance",
        krate: "mcrs_minecraft_world",
        value: ValueType::Defined("crate::test_types::TestInstance"),
    },
    Owner {
        registry: "minecraft:sulfur_cube_archetype",
        krate: "mcrs_minecraft_world",
        value: ValueType::Defined("crate::sulfur_cube_archetype::SulfurCubeArchetype"),
    },
    Owner {
        registry: "minecraft:enchantment_provider",
        krate: "mcrs_minecraft_world",
        value: ValueType::Defined("crate::enchantment_provider::EnchantmentProvider"),
    },
    Owner {
        registry: "minecraft:villager_trade",
        krate: "mcrs_minecraft_world",
        value: ValueType::Defined("crate::villager_trade::VillagerTrade"),
    },
    Owner {
        registry: "minecraft:trade_set",
        krate: "mcrs_minecraft_world",
        value: ValueType::Defined("crate::villager_trade::TradeSet"),
    },
    Owner {
        registry: "minecraft:worldgen/world_preset",
        krate: "mcrs_minecraft_world",
        value: ValueType::Defined("crate::worldgen::world_preset::WorldPreset"),
    },
    Owner {
        registry: "minecraft:worldgen/biome",
        krate: "mcrs_minecraft_biome",
        value: ValueType::Defined("crate::Biome"),
    },
    Owner {
        registry: "minecraft:worldgen/multi_noise_biome_source_parameter_list",
        krate: "mcrs_minecraft_biome",
        value: ValueType::Defined("crate::parameter_list::MultiNoiseBiomeSourceParameterList"),
    },
    Owner {
        registry: "minecraft:dimension",
        krate: "mcrs_minecraft_dimension",
        value: ValueType::Defined("crate::Dimension"),
    },
    Owner {
        registry: "minecraft:dimension_type",
        krate: "mcrs_minecraft_dimension",
        value: ValueType::Defined("crate::DimensionType"),
    },
    Owner {
        registry: "minecraft:worldgen/block_state_provider",
        krate: "mcrs_minecraft_block_predicate",
        value: ValueType::Defined("crate::provider::DirectBlockStateProvider"),
    },
    Owner {
        registry: "minecraft:worldgen/carver",
        krate: "mcrs_minecraft_worldgen_carver",
        value: ValueType::Defined("crate::config::CarverConfig"),
    },
    Owner {
        registry: "minecraft:worldgen/placed_feature",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Defined("crate::proto::PlacedFeature"),
    },
    Owner {
        registry: "minecraft:worldgen/feature",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Defined("crate::proto::Feature"),
    },
    Owner {
        registry: "minecraft:worldgen/processor_list",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Defined("crate::proto::StructureProcessorList"),
    },
    Owner {
        registry: "minecraft:worldgen/template_pool",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Defined("crate::pool::TemplatePool"),
    },
    Owner {
        registry: "minecraft:worldgen/structure",
        krate: "mcrs_minecraft_worldgen_structure",
        value: ValueType::Defined("crate::Structure"),
    },
    Owner {
        registry: "minecraft:worldgen/structure_set",
        krate: "mcrs_minecraft_worldgen_structure",
        value: ValueType::Defined("crate::StructureSet"),
    },
    Owner {
        registry: "minecraft:worldgen/noise_settings",
        krate: "mcrs_minecraft_worldgen_density",
        value: ValueType::Defined("crate::router::NoiseGeneratorSettings"),
    },
    Owner {
        registry: "minecraft:worldgen/noise",
        krate: "mcrs_minecraft_worldgen_noise",
        value: ValueType::Defined("crate::proto::NoiseParam"),
    },
    Owner {
        registry: "minecraft:worldgen/material_rule",
        krate: "mcrs_minecraft_worldgen_surface",
        value: ValueType::Defined("crate::proto::MaterialRule"),
    },
    Owner {
        registry: "minecraft:chunk_status",
        krate: "mcrs_minecraft_anvil",
        value: ValueType::Enum("ChunkStatus"),
    },
    Owner {
        registry: "minecraft:worldgen/feature_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum("FeatureType"),
    },
    Owner {
        registry: "minecraft:worldgen/placement_modifier_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum("PlacementModifierType"),
    },
    Owner {
        registry: "minecraft:worldgen/foliage_placer_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum("FoliagePlacerType"),
    },
    Owner {
        registry: "minecraft:worldgen/trunk_placer_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum("TrunkPlacerType"),
    },
    Owner {
        registry: "minecraft:worldgen/root_placer_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum("RootPlacerType"),
    },
    Owner {
        registry: "minecraft:worldgen/tree_decorator_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum("TreeDecoratorType"),
    },
    Owner {
        registry: "minecraft:worldgen/feature_size_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum("FeatureSizeType"),
    },
    Owner {
        registry: "minecraft:rule_test_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum("RuleTestType"),
    },
    Owner {
        registry: "minecraft:pos_rule_test",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum("PosRuleTestType"),
    },
    Owner {
        registry: "minecraft:rule_block_entity_modifier",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum("RuleBlockEntityModifierType"),
    },
    Owner {
        registry: "minecraft:worldgen/structure_processor",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum("StructureProcessorType"),
    },
    Owner {
        registry: "minecraft:worldgen/structure_pool_element",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum("StructurePoolElementType"),
    },
    Owner {
        registry: "minecraft:block_predicate_type",
        krate: "mcrs_minecraft_block_predicate",
        value: ValueType::Enum("BlockPredicateType"),
    },
    Owner {
        registry: "minecraft:worldgen/block_state_provider_type",
        krate: "mcrs_minecraft_block_predicate",
        value: ValueType::Enum("BlockStateProviderType"),
    },
    Owner {
        registry: "minecraft:worldgen/structure_type",
        krate: "mcrs_minecraft_worldgen_structure",
        value: ValueType::Enum("StructureType"),
    },
    Owner {
        registry: "minecraft:worldgen/structure_placement",
        krate: "mcrs_minecraft_worldgen_structure",
        value: ValueType::Enum("StructurePlacementType"),
    },
    Owner {
        registry: "minecraft:worldgen/structure_piece",
        krate: "mcrs_minecraft_worldgen_structure",
        value: ValueType::Enum("StructurePieceType"),
    },
    Owner {
        registry: "minecraft:worldgen/pool_alias_binding",
        krate: "mcrs_minecraft_worldgen_structure",
        value: ValueType::Enum("PoolAliasBindingType"),
    },
    Owner {
        registry: "minecraft:spawn_condition_type",
        krate: "mcrs_minecraft_worldgen_structure",
        value: ValueType::Enum("SpawnConditionType"),
    },
    Owner {
        registry: "minecraft:worldgen/material_condition_type",
        krate: "mcrs_minecraft_worldgen_surface",
        value: ValueType::Enum("MaterialConditionType"),
    },
    Owner {
        registry: "minecraft:worldgen/material_rule_type",
        krate: "mcrs_minecraft_worldgen_surface",
        value: ValueType::Enum("MaterialRuleType"),
    },
    Owner {
        registry: "minecraft:worldgen/density_function_type",
        krate: "mcrs_minecraft_worldgen_density",
        value: ValueType::Enum("DensityFunctionType"),
    },
    Owner {
        registry: "minecraft:worldgen/carver_type",
        krate: "mcrs_minecraft_worldgen_carver",
        value: ValueType::Enum("CarverType"),
    },
    Owner {
        registry: "minecraft:worldgen/biome_source",
        krate: "mcrs_minecraft_biome",
        value: ValueType::Enum("BiomeSourceType"),
    },
    Owner {
        registry: "minecraft:consume_effect_type",
        krate: "mcrs_minecraft_item",
        value: ValueType::Enum("ConsumeEffectType"),
    },
    Owner {
        registry: "minecraft:data_component_type",
        krate: "mcrs_minecraft_item",
        value: ValueType::Enum("DataComponentType"),
    },
    Owner {
        registry: "minecraft:data_component_predicate_type",
        krate: "mcrs_minecraft_item",
        value: ValueType::Enum("DataComponentPredicateType"),
    },
    Owner {
        registry: "minecraft:dialog_type",
        krate: "mcrs_minecraft_item",
        value: ValueType::Enum("DialogType"),
    },
    Owner {
        registry: "minecraft:dialog_body_type",
        krate: "mcrs_minecraft_item",
        value: ValueType::Enum("DialogBodyType"),
    },
    Owner {
        registry: "minecraft:input_control_type",
        krate: "mcrs_minecraft_item",
        value: ValueType::Enum("InputControlType"),
    },
    Owner {
        registry: "minecraft:map_decoration_type",
        krate: "mcrs_minecraft_item",
        value: ValueType::Enum("MapDecorationType"),
    },
    Owner {
        registry: "minecraft:potion",
        krate: "mcrs_minecraft_item",
        value: ValueType::Enum("Potion"),
    },
    Owner {
        registry: "minecraft:mob_effect",
        krate: "mcrs_minecraft_item",
        value: ValueType::Enum("MobEffect"),
    },
    Owner {
        registry: "minecraft:villager_type",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Enum("VillagerType"),
    },
    Owner {
        registry: "minecraft:villager_profession",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Enum("VillagerProfession"),
    },
    Owner {
        registry: "minecraft:enchantment_effect_component_type",
        krate: "mcrs_minecraft_enchantment",
        value: ValueType::Enum("EnchantmentEffectComponentType"),
    },
    Owner {
        registry: "minecraft:enchantment_entity_effect_type",
        krate: "mcrs_minecraft_enchantment",
        value: ValueType::Enum("EnchantmentEntityEffectType"),
    },
    Owner {
        registry: "minecraft:enchantment_level_based_value_type",
        krate: "mcrs_minecraft_enchantment",
        value: ValueType::Enum("EnchantmentLevelBasedValueType"),
    },
    Owner {
        registry: "minecraft:enchantment_location_based_effect_type",
        krate: "mcrs_minecraft_enchantment",
        value: ValueType::Enum("EnchantmentLocationBasedEffectType"),
    },
    Owner {
        registry: "minecraft:enchantment_value_effect_type",
        krate: "mcrs_minecraft_enchantment",
        value: ValueType::Enum("EnchantmentValueEffectType"),
    },
    Owner {
        registry: "minecraft:loot_condition_type",
        krate: "mcrs_minecraft_enchantment",
        value: ValueType::Enum("LootConditionType"),
    },
    Owner {
        registry: "minecraft:particle_type",
        krate: "mcrs_minecraft_particle",
        value: ValueType::Enum("ParticleType"),
    },
    Owner {
        registry: "minecraft:position_source_type",
        krate: "mcrs_minecraft_particle",
        value: ValueType::Enum("PositionSourceType"),
    },
    Owner {
        registry: "minecraft:recipe_display",
        krate: "mcrs_minecraft_protocol",
        value: ValueType::Enum("RecipeDisplayType"),
    },
    Owner {
        registry: "minecraft:slot_display",
        krate: "mcrs_minecraft_protocol",
        value: ValueType::Enum("SlotDisplayType"),
    },
    Owner {
        registry: "minecraft:activity",
        krate: "mcrs_minecraft_environment",
        value: ValueType::Enum("Activity"),
    },
    Owner {
        registry: "minecraft:environment_attribute",
        krate: "mcrs_minecraft_environment",
        value: ValueType::Enum("EnvironmentAttribute"),
    },
    Owner {
        registry: "minecraft:entity_type",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Enum("EntityType"),
    },
    Owner {
        registry: "minecraft:attribute",
        krate: "mcrs_minecraft_entity",
        value: ValueType::Enum("Attribute"),
    },
    Owner {
        registry: "minecraft:menu",
        krate: "mcrs_minecraft_item",
        value: ValueType::Enum("MenuType"),
    },
    Owner {
        registry: "minecraft:block",
        krate: "mcrs_minecraft_block",
        value: ValueType::Enum("Block"),
    },
    Owner {
        registry: "minecraft:fluid",
        krate: "mcrs_minecraft_block",
        value: ValueType::Enum("Fluid"),
    },
    Owner {
        registry: "minecraft:block_entity_type",
        krate: "mcrs_minecraft_block",
        value: ValueType::Enum("BlockEntityType"),
    },
    Owner {
        registry: "minecraft:item",
        krate: "mcrs_minecraft_item",
        value: ValueType::Enum("Item"),
    },
    Owner {
        registry: "minecraft:int_provider_type",
        krate: "mcrs_minecraft_value_provider",
        value: ValueType::Enum("IntProviderType"),
    },
    Owner {
        registry: "minecraft:float_provider_type",
        krate: "mcrs_minecraft_value_provider",
        value: ValueType::Enum("FloatProviderType"),
    },
    Owner {
        registry: "minecraft:height_provider_type",
        krate: "mcrs_minecraft_value_provider",
        value: ValueType::Enum("HeightProviderType"),
    },
];
