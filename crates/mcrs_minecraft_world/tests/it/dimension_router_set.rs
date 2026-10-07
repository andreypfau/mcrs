use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block_predicate::block_state::BlockState;
use mcrs_minecraft_world::registries::test_registries;
use mcrs_minecraft_worldgen::router::{build_dimension_router, dimension_beardifier_placement};
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use std::cell::RefCell;
use std::collections::BTreeMap;

#[test]
fn every_noise_settings_compiles_into_a_router_from_the_loaded_set() {
    let set = test_registries();
    let biomes = set.tags::<Biome>().expect("the set holds the biome tags");
    let registry = set
        .registry::<NoiseGeneratorSettings>()
        .expect("the set holds the noise settings registry");
    let values = set
        .entries::<NoiseGeneratorSettings, NoiseGeneratorSettings>()
        .expect("the set parses the noise settings");
    assert_eq!(registry.ids().count(), 8);

    for id in registry.ids() {
        let name = registry.name(id).expect("an id of the registry has a name");
        let settings = values.get(id).expect("a value for every id");

        // Every distinct state gets a distinct id, so a router that mixed the
        // terrain block up with the sea fluid would not compare equal.
        let states = RefCell::new(BTreeMap::<String, u16>::new());
        let block = |state: &BlockState| {
            let mut states = states.borrow_mut();
            let next = states.len() as u16 + 1;
            Some((*states.entry(state.name.as_str().to_owned()).or_insert(next)).into())
        };
        let (router, _) = build_dimension_router(set, settings, 0, &block, &biomes)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_ne!(
            router.default_block_state, router.default_fluid_state,
            "{name} resolved the terrain block and the sea fluid to one id"
        );
        dimension_beardifier_placement(set, settings);
    }
}
