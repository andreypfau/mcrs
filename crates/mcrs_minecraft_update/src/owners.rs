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
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/feature_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/placement_modifier_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/foliage_placer_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/trunk_placer_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/root_placer_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/tree_decorator_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/feature_size_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:rule_test_type",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:pos_rule_test",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:rule_block_entity_modifier",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/structure_processor",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/structure_pool_element",
        krate: "mcrs_minecraft_worldgen_feature",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:block_predicate_type",
        krate: "mcrs_minecraft_block_predicate",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/block_state_provider_type",
        krate: "mcrs_minecraft_block_predicate",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/structure_type",
        krate: "mcrs_minecraft_worldgen_structure",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/structure_placement",
        krate: "mcrs_minecraft_worldgen_structure",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/structure_piece",
        krate: "mcrs_minecraft_worldgen_structure",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/pool_alias_binding",
        krate: "mcrs_minecraft_worldgen_structure",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:spawn_condition_type",
        krate: "mcrs_minecraft_worldgen_structure",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/material_condition_type",
        krate: "mcrs_minecraft_worldgen_surface",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/material_rule_type",
        krate: "mcrs_minecraft_worldgen_surface",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/density_function_type",
        krate: "mcrs_minecraft_worldgen_density",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/carver_type",
        krate: "mcrs_minecraft_worldgen_carver",
        value: ValueType::Enum,
    },
    Owner {
        registry: "minecraft:worldgen/biome_source",
        krate: "mcrs_minecraft_biome",
        value: ValueType::Enum,
    },
];
