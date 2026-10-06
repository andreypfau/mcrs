use crate::keys::Owner;

pub const OWNERS: &[Owner] = &[
    Owner {
        registry: "minecraft:timeline",
        krate: "mcrs_minecraft_environment",
        value: "crate::timeline::Timeline",
    },
    Owner {
        registry: "minecraft:world_clock",
        krate: "mcrs_minecraft_environment",
        value: "crate::world_clock::WorldClock",
    },
    Owner {
        registry: "minecraft:banner_pattern",
        krate: "mcrs_minecraft_item",
        value: "crate::BannerPattern",
    },
    Owner {
        registry: "minecraft:instrument",
        krate: "mcrs_minecraft_item",
        value: "crate::InstrumentValue",
    },
    Owner {
        registry: "minecraft:jukebox_song",
        krate: "mcrs_minecraft_item",
        value: "crate::JukeboxSong",
    },
    Owner {
        registry: "minecraft:painting_variant",
        krate: "mcrs_minecraft_item",
        value: "crate::PaintingVariantValue",
    },
    Owner {
        registry: "minecraft:trim_material",
        krate: "mcrs_minecraft_item",
        value: "crate::TrimMaterial",
    },
    Owner {
        registry: "minecraft:trim_pattern",
        krate: "mcrs_minecraft_item",
        value: "crate::TrimPattern",
    },
    Owner {
        registry: "minecraft:sound_event",
        krate: "mcrs_minecraft_sound",
        value: "crate::SoundEvent",
    },
    Owner {
        registry: "minecraft:damage_type",
        krate: "mcrs_minecraft_item",
        value: "crate::damage_type::DamageType",
    },
    Owner {
        registry: "minecraft:decorated_pot_pattern",
        krate: "mcrs_minecraft_item",
        value: "crate::decorated_pot_pattern::DecoratedPotPattern",
    },
    Owner {
        registry: "minecraft:dialog",
        krate: "mcrs_minecraft_item",
        value: "crate::dialog::Dialog",
    },
    Owner {
        registry: "minecraft:block_transformer",
        krate: "mcrs_minecraft_item",
        value: "crate::block_transformer::BlockTransformer",
    },
    Owner {
        registry: "minecraft:enchantment",
        krate: "mcrs_minecraft_item",
        value: "crate::enchantment::EnchantmentData",
    },
    Owner {
        registry: "minecraft:wolf_variant",
        krate: "mcrs_minecraft_entity",
        value: "crate::variant::WolfVariant",
    },
    Owner {
        registry: "minecraft:pig_variant",
        krate: "mcrs_minecraft_entity",
        value: "crate::variant::PigVariant",
    },
    Owner {
        registry: "minecraft:cow_variant",
        krate: "mcrs_minecraft_entity",
        value: "crate::variant::CowVariant",
    },
    Owner {
        registry: "minecraft:chicken_variant",
        krate: "mcrs_minecraft_entity",
        value: "crate::variant::ChickenVariant",
    },
    Owner {
        registry: "minecraft:cat_variant",
        krate: "mcrs_minecraft_entity",
        value: "crate::variant::CatVariant",
    },
    Owner {
        registry: "minecraft:frog_variant",
        krate: "mcrs_minecraft_entity",
        value: "crate::variant::FrogVariant",
    },
    Owner {
        registry: "minecraft:zombie_nautilus_variant",
        krate: "mcrs_minecraft_entity",
        value: "crate::variant::ZombieNautilusVariant",
    },
    Owner {
        registry: "minecraft:wolf_sound_variant",
        krate: "mcrs_minecraft_entity",
        value: "crate::variant::WolfSoundVariant",
    },
    Owner {
        registry: "minecraft:pig_sound_variant",
        krate: "mcrs_minecraft_entity",
        value: "crate::variant::PigSoundVariant",
    },
    Owner {
        registry: "minecraft:cow_sound_variant",
        krate: "mcrs_minecraft_entity",
        value: "crate::variant::CowSoundVariant",
    },
    Owner {
        registry: "minecraft:chicken_sound_variant",
        krate: "mcrs_minecraft_entity",
        value: "crate::variant::ChickenSoundVariant",
    },
    Owner {
        registry: "minecraft:cat_sound_variant",
        krate: "mcrs_minecraft_entity",
        value: "crate::variant::CatSoundVariant",
    },
    Owner {
        registry: "minecraft:chat_type",
        krate: "mcrs_minecraft_world",
        value: "crate::chat_type::ChatType",
    },
    Owner {
        registry: "minecraft:test_environment",
        krate: "mcrs_minecraft_world",
        value: "crate::test_types::TestEnvironment",
    },
    Owner {
        registry: "minecraft:test_instance",
        krate: "mcrs_minecraft_world",
        value: "crate::test_types::TestInstance",
    },
    Owner {
        registry: "minecraft:sulfur_cube_archetype",
        krate: "mcrs_minecraft_world",
        value: "crate::sulfur_cube_archetype::SulfurCubeArchetype",
    },
    Owner {
        registry: "minecraft:enchantment_provider",
        krate: "mcrs_minecraft_world",
        value: "crate::enchantment_provider::EnchantmentProvider",
    },
    Owner {
        registry: "minecraft:villager_trade",
        krate: "mcrs_minecraft_world",
        value: "crate::villager_trade::VillagerTrade",
    },
    Owner {
        registry: "minecraft:trade_set",
        krate: "mcrs_minecraft_world",
        value: "crate::villager_trade::TradeSet",
    },
    Owner {
        registry: "minecraft:worldgen/world_preset",
        krate: "mcrs_minecraft_world",
        value: "crate::worldgen::world_preset::WorldPreset",
    },
    Owner {
        registry: "minecraft:worldgen/biome",
        krate: "mcrs_minecraft_biome",
        value: "crate::Biome",
    },
    Owner {
        registry: "minecraft:worldgen/multi_noise_biome_source_parameter_list",
        krate: "mcrs_minecraft_biome",
        value: "crate::parameter_list::MultiNoiseBiomeSourceParameterList",
    },
    Owner {
        registry: "minecraft:dimension",
        krate: "mcrs_minecraft_dimension",
        value: "crate::Dimension",
    },
    Owner {
        registry: "minecraft:dimension_type",
        krate: "mcrs_minecraft_dimension",
        value: "crate::DimensionType",
    },
    Owner {
        registry: "minecraft:worldgen/block_state_provider",
        krate: "mcrs_minecraft_block_predicate",
        value: "crate::provider::DirectBlockStateProvider",
    },
];
