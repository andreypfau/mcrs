use super::corpus_biomes;
use crate::SurfaceIds;
use crate::ids::{FillIds, require_id};
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_game_registries::registry_load::{registries_declaring, test_registries};
use mcrs_minecraft_registries::{Declaration, Diagnostics, Registries};
use mcrs_minecraft_resource::Registered;

fn biomes_without(skipped: &str) -> Registries {
    let biomes = corpus_biomes();
    let names = biomes
        .iter()
        .map(|(_, name, _)| name)
        .filter(|name| name.as_str() != skipped)
        .cloned()
        .collect();
    registries_declaring(vec![Declaration::fixed(
        mcrs_minecraft_biome::keys::BIOME.location().into(),
        names,
    )])
}

#[test]
fn the_corpus_holds_every_name_the_generator_resolves() {
    let registries = test_registries();
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
    let mut diagnostics = Diagnostics::default();
    assert!(
        SurfaceIds::from_lookup(|key| require_id(&registries, key, &mut diagnostics)).is_none()
    );
    let text = diagnostics.to_string();
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("minecraft:worldgen/biome"), "{text}");
    assert!(text.contains("minecraft:eroded_badlands"), "{text}");
}

#[test]
fn a_block_tag_the_packs_leave_out_is_empty_rather_than_an_error() {
    let blocks = Block::ENTRIES.iter().map(|&name| name.into()).collect();
    let registries = registries_declaring(vec![
        Declaration::fixed(Block::REGISTRY.location().into(), blocks),
        Declaration::fixed(
            mcrs_minecraft_biome::keys::BIOME.location().into(),
            corpus_biomes()
                .iter()
                .map(|(_, name, _)| name.clone())
                .collect(),
        ),
    ]);
    let mut diagnostics = Diagnostics::default();
    let ids = FillIds::resolve(&registries, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics}");
    assert_eq!(ids.map(|ids| ids.uncarvable), Some(None));
}
