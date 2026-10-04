use mcrs_minecraft_biome::Biome;

pub(super) fn make_beta_biome() -> Biome {
    Biome {
        temperature: 0.5,
        downfall: 0.5,
        has_precipitation: true,
        temperature_modifier: None,
        effects: mcrs_minecraft_biome::BiomeEffects {
            water_color: None,
            foliage_color: None,
            grass_color: None,
            grass_color_modifier: Default::default(),
            dry_foliage_color: None,
        },
        carvers: Vec::new(),
        features: Vec::new(),
        attributes: Default::default(),
    }
}
