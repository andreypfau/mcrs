use mcrs_minecraft_block_predicate::block_state::BlockState;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_worldgen::router::{build_dimension_router, dimension_beardifier_placement};
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use std::cell::RefCell;
use std::collections::BTreeMap;

#[test]
fn every_noise_settings_compiles_into_a_router_from_the_loaded_set() {
    let set = test_registries();
    let registry = set
        .registry::<NoiseGeneratorSettings>()
        .expect("the set holds the noise settings registry");
    assert_eq!(registry.len(), 8);

    for (_, name, settings) in registry.iter() {
        // Every distinct state gets a distinct id, so a router that mixed the
        // terrain block up with the sea fluid would not compare equal.
        let states = RefCell::new(BTreeMap::<String, u16>::new());
        let block = |state: &BlockState| {
            let mut states = states.borrow_mut();
            let next = states.len() as u16 + 1;
            Some((*states.entry(state.name.as_str().to_owned()).or_insert(next)).into())
        };
        let (router, _) = build_dimension_router(set, settings, 0, &block)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_ne!(
            router.default_block_state, router.default_fluid_state,
            "{name} resolved the terrain block and the sea fluid to one id"
        );
        dimension_beardifier_placement(set, settings);
    }
}
