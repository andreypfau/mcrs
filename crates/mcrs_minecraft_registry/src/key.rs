mcrs_minecraft_core::registry_keys! {
    /// The block registry, as the tag system names it. The blocks themselves live
    /// in the definition corpus and are addressed by their index in it, so this
    /// type carries no value — it only says which registry a `TagKey` belongs to.
    Block = "minecraft:block", tags "block";
    /// The fluid registry, as the tag system names it. The fluids are the ones the
    /// block corpus interns, addressed by their index there.
    Fluid = "minecraft:fluid", tags "fluid";
    Dialog = "minecraft:dialog", tags "dialog";
    Biome = "minecraft:worldgen/biome", tags "worldgen/biome";
    Structure = "minecraft:worldgen/structure", tags "worldgen/structure";
    BlockTransformer = "minecraft:block_transformer";
    DecoratedPotPattern = "minecraft:decorated_pot_pattern";
    BlockEntityType = "minecraft:block_entity_type";
    Dimension = "minecraft:dimension";
    ParticleType = "minecraft:particle_type";
    Carver = "minecraft:worldgen/carver";
    EnvironmentAttribute = "minecraft:environment_attribute";
    Activity = "minecraft:activity";
    DimensionType = "minecraft:dimension_type";
    TemplatePool = "minecraft:worldgen/template_pool";
    MaterialRule = "minecraft:worldgen/material_rule";
    BlockStateProvider = "minecraft:worldgen/block_state_provider";
    MultiNoiseBiomeSourceParameterList = "minecraft:worldgen/multi_noise_biome_source_parameter_list";
    VillagerProfession = "minecraft:villager_profession";
    ContextKeySet = "minecraft:context_key_set";
    TestFunction = "minecraft:test_function";
    TestInstanceType = "minecraft:test_instance_type";
    TestEnvironmentDefinitionType = "minecraft:test_environment_definition_type";
}
