use super::corpus_biomes;
use crate::SurfaceIds;
use crate::ids::{FillIds, SurvivalIds};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_registry::{LoadReport, Registry, RegistrySet};
use mcrs_minecraft_worldgen_testing::corpus_set;

fn biomes_without(skipped: &str) -> RegistrySet {
    let biomes = corpus_biomes();
    let names = biomes
        .ids()
        .filter_map(|id| biomes.name(id))
        .filter(|name| name.as_str() != skipped)
        .cloned();
    RegistrySet::new()
        .with(
            Registry::<Biome>::new(mcrs_minecraft_biome::keys::BIOME, names)
                .expect("the corpus names distinct biomes"),
        )
        .expect("one biome registry")
}

#[test]
fn the_corpus_holds_every_name_the_generator_resolves() {
    let set = corpus_set();
    let mut report = LoadReport::new();
    assert!(SurfaceIds::resolve(set, &mut report).is_some(), "{report}");
    assert!(FillIds::resolve(set, &mut report).is_some(), "{report}");
    assert!(SurvivalIds::resolve(set, &mut report).is_some(), "{report}");
    assert!(report.is_empty(), "{report}");
}

#[test]
fn a_biome_the_surface_names_and_the_registry_lacks_is_the_only_line_reported() {
    let set = biomes_without("minecraft:eroded_badlands");
    let mut report = LoadReport::new();
    assert!(SurfaceIds::resolve(&set, &mut report).is_none());
    let text = report.to_string();
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("minecraft:worldgen/biome"), "{text}");
    assert!(text.contains("minecraft:eroded_badlands"), "{text}");
}

#[test]
fn a_tag_the_survival_rules_name_and_the_set_lacks_is_reported_by_registry_and_tag() {
    let set = biomes_without("minecraft:plains");
    let mut report = LoadReport::new();
    assert!(SurvivalIds::resolve(&set, &mut report).is_none());
    let text = report.to_string();
    assert!(
        text.lines().all(|line| line.starts_with("minecraft:root/")),
        "{text}"
    );
    assert!(text.contains("minecraft:block"), "{text}");
    assert!(text.contains("minecraft:fluid"), "{text}");
}
