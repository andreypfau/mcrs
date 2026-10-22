use super::corpus_biomes;
use crate::SurfaceIds;
use crate::ids::FillIds;
use mcrs_minecraft_registries::require_id;
use mcrs_minecraft_registries::{Declaration, Diagnostics, Registries, RegistryType};
use mcrs_minecraft_world::registry_load::{registries_declaring, test_registries_arc};

struct TestBiome;

impl RegistryType for TestBiome {
    type Value = ();
}

fn biomes_without(skipped: &str) -> Registries {
    let biomes = corpus_biomes();
    let names = biomes
        .iter()
        .map(|(_, name, _)| name)
        .filter(|name| name.as_str() != skipped)
        .cloned()
        .collect();
    registries_declaring(&[Declaration::fixed::<TestBiome>(
        mcrs_minecraft_biome::keys::BIOME.location().into(),
        names,
    )])
}

#[test]
fn the_corpus_holds_every_name_the_generator_resolves() {
    let registries = test_registries_arc();
    let mut diagnostics = Diagnostics::default();
    assert!(
        SurfaceIds::resolve(registries, &mut diagnostics).is_some(),
        "{diagnostics}"
    );
    assert!(
        FillIds::resolve(registries, &mut diagnostics).is_some(),
        "{diagnostics}"
    );
    assert!(diagnostics.is_empty(), "{diagnostics}");
}

#[test]
fn a_biome_the_surface_names_and_the_registry_lacks_is_the_only_line_reported() {
    let registries = biomes_without("minecraft:eroded_badlands");
    let biomes = registries
        .registry::<TestBiome>()
        .expect("the registry was declared");
    let mut diagnostics = Diagnostics::default();
    assert!(SurfaceIds::from_lookup(|key| require_id(biomes, key, &mut diagnostics)).is_none());
    let text = diagnostics.to_string();
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("minecraft:worldgen/biome"), "{text}");
    assert!(text.contains("minecraft:eroded_badlands"), "{text}");
}
