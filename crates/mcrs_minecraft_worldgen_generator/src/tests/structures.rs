use mcrs_minecraft_worldgen_testing::registry;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, LazyLock};

use fixedbitset::FixedBitSet;
use mcrs_minecraft_biome::source::{BiomeSource, MultiNoiseBiomeSource};
use mcrs_minecraft_core::rl;
use mcrs_minecraft_core::{ResourceLocation, VERSION};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes;
use mcrs_minecraft_registry::{Registry, TagRules, Tags, build_tags};
use mcrs_minecraft_worldgen_feature::spawn_condition::SpawnSelector;
use mcrs_minecraft_worldgen_feature::template::Projection;
use mcrs_minecraft_worldgen_feature::template::{
    PaletteState, ResolvedState, Template, TemplateBlock,
};
use mcrs_minecraft_worldgen_structure::{
    MineshaftType, OceanTemperature, Structure, StructureSet, TemplatePool,
};
use mcrs_minecraft_worldgen_testing::{assets_dir, corpus_set, json_files};

use super::{biome_tags, corpus, corpus_biomes, structure_registry, structure_tags};
use crate::features::possible_biomes;
use crate::structures::{StructureInputs, VariantInputs, freeze, live_sets, resolve_palette_state};
use mcrs_minecraft_worldgen_structure::frozen::{FrozenElement, FrozenStructures, StructureKind};

pub(super) fn template_file<'a>(id: &ResourceLocation) -> Option<Cow<'a, Template>> {
    let bytes = mcrs_minecraft_worldgen_testing::template(id)?;
    let template = from_gzip_bytes(bytes.as_slice()).unwrap_or_else(|e| panic!("{id}: {e}"));
    Some(Cow::Owned(template))
}

struct Corpus {
    sets: BTreeMap<ResourceLocation, StructureSet>,
    structures: BTreeMap<ResourceLocation, Structure>,
    pools: BTreeMap<ResourceLocation, TemplatePool>,
}

fn corpus_registries() -> Corpus {
    Corpus {
        sets: registry("structure_set"),
        structures: registry("structure"),
        pools: registry("template_pool"),
    }
}

/// One variant registry's assets by id in registry order, which is the
/// alphabetical order the snapshot assigns; only the spawn conditions are read.
fn variant_selectors(registry: &str) -> BTreeMap<ResourceLocation, Vec<SpawnSelector>> {
    #[derive(serde::Deserialize)]
    struct Variant {
        #[serde(default)]
        spawn_conditions: Vec<SpawnSelector>,
    }
    let base = assets_dir().join("minecraft").join(registry);
    json_files(&base)
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let variant: Variant = corpus_set()
                .scope(|| serde_json::from_slice(&bytes))
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let name = path.file_stem().expect("a file").to_string_lossy();
            (
                ResourceLocation::minecraft(&name).unwrap(),
                variant.spawn_conditions,
            )
        })
        .collect()
}

pub(super) fn variant_inputs() -> VariantInputs<'static> {
    static CATS: LazyLock<BTreeMap<ResourceLocation, Vec<SpawnSelector>>> =
        LazyLock::new(|| variant_selectors("cat_variant"));
    static CAT_SOUNDS: LazyLock<Vec<ResourceLocation>> =
        LazyLock::new(|| variant_selectors("cat_sound_variant").into_keys().collect());
    static CHICKENS: LazyLock<BTreeMap<ResourceLocation, Vec<SpawnSelector>>> =
        LazyLock::new(|| variant_selectors("chicken_variant"));
    static CHICKEN_SOUNDS: LazyLock<Vec<ResourceLocation>> = LazyLock::new(|| {
        variant_selectors("chicken_sound_variant")
            .into_keys()
            .collect()
    });
    static ZOMBIE_NAUTILUSES: LazyLock<BTreeMap<ResourceLocation, Vec<SpawnSelector>>> =
        LazyLock::new(|| variant_selectors("zombie_nautilus_variant"));
    VariantInputs {
        cats: Some(&CATS),
        cat_sounds: &CAT_SOUNDS,
        chickens: Some(&CHICKENS),
        chicken_sounds: &CHICKEN_SOUNDS,
        zombie_nautiluses: Some(&ZOMBIE_NAUTILUSES),
    }
}

pub(super) fn frozen() -> &'static FrozenStructures {
    frozen_shared()
}

pub(super) fn frozen_shared() -> &'static Arc<FrozenStructures> {
    static FROZEN: LazyLock<Arc<FrozenStructures>> = LazyLock::new(|| {
        let corpus_registries = corpus_registries();
        freeze(&StructureInputs {
            sets: &corpus_registries.sets,
            structures: &corpus_registries.structures,
            pools: &corpus_registries.pools,
            template: &template_file,
            resolve: &|state| resolve_palette_state(corpus(), state),
            biomes: corpus_biomes(),
            biome_tags: &biome_tags(),
            structure_registry: structure_registry(),
            structure_tags: &structure_tags(),
            variants: &variant_inputs(),
        })
        .unwrap_or_else(|e| panic!("{e}"))
        .into()
    });
    &FROZEN
}

fn structure(id: &str) -> &'static mcrs_minecraft_worldgen_structure::frozen::FrozenStructure {
    let frozen = frozen();
    let id = ResourceLocation::read(id).unwrap();
    &frozen.structures[usize::from(frozen.structure_ids[&id].0)]
}

#[test]
fn the_corpus_freezes() {
    the_cat_variants_freeze_with_the_swamp_hut_in_their_structure_tag();
    every_hardcoded_type_freezes_its_own_config();
    every_template_a_structure_names_is_loaded_and_the_pools_expand();
    each_dimension_source_keeps_the_sets_its_biomes_admit();
}

fn the_cat_variants_freeze_with_the_swamp_hut_in_their_structure_tag() {
    use mcrs_minecraft_random::worldgen::WorldgenRandom;
    use mcrs_minecraft_worldgen_feature::spawn_condition::SpawnContext;

    let frozen = frozen();
    let variants = &frozen.variants;
    assert_eq!(variants.cats.ids.len(), 11);
    assert_eq!(variants.cats.ids[0], rl!("minecraft:all_black").to_arc());
    assert_eq!(
        variants.cat_sounds,
        vec![
            rl!("minecraft:classic").to_arc(),
            rl!("minecraft:royal").to_arc(),
        ]
    );
    let in_hut = SpawnContext {
        structure: Some(frozen.structure_ids[&rl!("minecraft:swamp_hut").to_arc()].0),
        biome: corpus_biomes().by_name("minecraft:swamp").unwrap().number(),
        moon_brightness: 1.0,
    };
    let mut rng = WorldgenRandom::new(1);
    for _ in 0..20 {
        assert_eq!(
            variants.cats.pick(&in_hut, &mut rng),
            Some(&rl!("minecraft:all_black").to_arc())
        );
    }
    let elsewhere = SpawnContext {
        structure: Some(frozen.structure_ids[&rl!("minecraft:igloo").to_arc()].0),
        moon_brightness: 0.0,
        ..in_hut
    };
    let picked: BTreeSet<_> = (0..200)
        .map(|_| variants.cats.pick(&elsewhere, &mut rng).unwrap().clone())
        .collect();
    assert_eq!(picked.len(), 10);
    assert!(!picked.contains(&rl!("minecraft:all_black").to_arc()));
}

fn every_hardcoded_type_freezes_its_own_config() {
    let biome = |name: &str| corpus_biomes().by_name(name).unwrap().index();
    let StructureKind::Mineshaft {
        mineshaft_type,
        blocking,
    } = &structure("minecraft:mineshaft_mesa").kind
    else {
        panic!("not a mineshaft");
    };
    assert_eq!(*mineshaft_type, MineshaftType::Mesa);
    assert!(blocking.contains(biome("minecraft:deep_dark")));
    assert!(!blocking.contains(biome("minecraft:plains")));

    let StructureKind::OceanMonument { surrounding } = &structure("minecraft:monument").kind else {
        panic!("not a monument");
    };
    assert!(surrounding.contains(biome("minecraft:deep_ocean")));
    assert!(surrounding.contains(biome("minecraft:river")));
    assert!(!surrounding.contains(biome("minecraft:plains")));

    let StructureKind::OceanRuin(config) = &structure("minecraft:ocean_ruin_warm").kind else {
        panic!("not an ocean ruin");
    };
    assert_eq!(config.biome_temp, OceanTemperature::Warm);
    assert_eq!(
        (config.large_probability, config.cluster_probability),
        (0.3, 0.9)
    );

    let StructureKind::RuinedPortal {
        setups,
        portals,
        giant_portals,
    } = &structure("minecraft:ruined_portal").kind
    else {
        panic!("not a ruined portal");
    };
    assert_eq!(setups.len(), 2);
    assert!(setups[0].can_be_cold && !setups[0].replace_with_blackstone);
    assert_eq!((portals.len(), giant_portals.len()), (10, 3));
    let portal_sizes: Vec<[u16; 3]> = portals
        .iter()
        .map(|id| frozen().manifests[id.0 as usize].size)
        .collect();
    assert!(portal_sizes.iter().all(|size| *size != [0; 3]));

    let StructureKind::Shipwreck {
        is_beached,
        templates,
    } = &structure("minecraft:shipwreck_beached").kind
    else {
        panic!("not a shipwreck");
    };
    assert!(is_beached);
    assert_eq!(templates.len(), 11);
    let StructureKind::Shipwreck { templates, .. } = &structure("minecraft:shipwreck").kind else {
        panic!("not a shipwreck");
    };
    assert_eq!(templates.len(), 20);
    assert!(matches!(
        structure("minecraft:nether_fossil").kind,
        StructureKind::NetherFossil { .. }
    ));
}

fn every_template_a_structure_names_is_loaded_and_the_pools_expand() {
    let frozen = frozen();
    for structure in corpus_registries().structures.values() {
        for path in structure.templates() {
            let id = frozen.template_ids[&ResourceLocation::minecraft(path).unwrap()];
            assert_ne!(frozen.templates[id.0 as usize].size, [0; 3], "{path}");
        }
    }
    let missing =
        ResourceLocation::read("minecraft:ancient_city/walls/intact_horizontal_wall_stairs_5")
            .unwrap();
    let substitute = &frozen.templates[frozen.template_ids[&missing].0 as usize];
    assert_eq!(substitute.size, [0; 3]);
    assert!(substitute.palettes.is_empty());
    assert_eq!(
        frozen
            .templates
            .iter()
            .filter(|template| template.size == [0; 3])
            .count(),
        1
    );

    assert_eq!(structure("minecraft:village_plains").step_index, 40);
    assert_eq!(structure("minecraft:ancient_city").step_index, 0);

    let empty = frozen.pool_ids[&rl!("minecraft:empty").to_arc()];
    let empty_pool = &frozen.pools[usize::from(empty.0)];
    assert_eq!(empty_pool.fallback, empty);
    assert!(empty_pool.expanded.is_empty());
    assert_eq!(empty_pool.max_size, 0);
    let houses = &frozen.pools
        [usize::from(frozen.pool_ids[&rl!("minecraft:village/plains/houses").to_arc()].0)];
    assert!(
        houses.expanded.len()
            > houses
                .expanded
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
    );
    assert!(houses.max_size > 1);
}

fn live_set_names(source: &BiomeSource) -> Vec<String> {
    let frozen = frozen();
    let mut mask = FixedBitSet::with_capacity(corpus_biomes().len());
    for name in possible_biomes(source, corpus_biomes(), &crate::tests::parameter_lists().1) {
        mask.insert(corpus_biomes().by_name(name.as_str()).unwrap().index());
    }
    live_sets(frozen, &mask)
        .into_iter()
        .map(|(set, _)| frozen.sets[usize::from(set.0)].id.to_string())
        .collect()
}

pub(super) fn preset(name: &str) -> BiomeSource {
    BiomeSource::MultiNoise(MultiNoiseBiomeSource {
        preset: Some(crate::tests::parameter_list_id(name)),
        biomes: None,
    })
}

fn each_dimension_source_keeps_the_sets_its_biomes_admit() {
    let every: Vec<String> = frozen().sets.iter().map(|set| set.id.to_string()).collect();
    let overworld: Vec<String> = every
        .iter()
        .filter(|id| {
            !matches!(
                id.as_str(),
                "minecraft:end_cities" | "minecraft:nether_complexes" | "minecraft:nether_fossils"
            )
        })
        .cloned()
        .collect();
    assert_eq!(live_set_names(&preset("minecraft:overworld")), overworld);
    assert_eq!(
        live_set_names(&preset("minecraft:nether")),
        [
            "minecraft:nether_complexes",
            "minecraft:nether_fossils",
            "minecraft:ruined_portals"
        ]
    );
    assert_eq!(
        live_set_names(&BiomeSource::TheEnd),
        ["minecraft:end_cities"]
    );
}

fn parse<T: serde::de::DeserializeOwned>(
    entries: &[(&str, &str)],
) -> BTreeMap<ResourceLocation, T> {
    entries
        .iter()
        .map(|(id, json)| {
            (
                ResourceLocation::read(id).unwrap(),
                corpus_set()
                    .scope(|| serde_json::from_str(json))
                    .unwrap_or_else(|e| panic!("{id}: {e}")),
            )
        })
        .collect()
}

fn no_tags<R: 'static>(registry: &Registry<R>) -> Tags<R> {
    let (table, problems) = build_tags(registry.table(), TagRules::World, &[], None);
    assert!(problems.is_empty(), "{problems:?}");
    Tags::new(Arc::new(table))
}

/// Freeze a synthetic corpus with no templates, biomes or tags loaded.
fn try_freeze(
    sets: &[(&str, &str)],
    structures: &[(&str, &str)],
    pools: &[(&str, &str)],
) -> Result<FrozenStructures, String> {
    try_freeze_with(sets, structures, pools, |_| None, &|_| None)
}

fn try_freeze_with(
    sets: &[(&str, &str)],
    structures: &[(&str, &str)],
    pools: &[(&str, &str)],
    template: impl Fn(&ResourceLocation) -> Option<Template>,
    resolve: &dyn Fn(&PaletteState) -> Option<ResolvedState>,
) -> Result<FrozenStructures, String> {
    let sets = parse::<StructureSet>(sets);
    let structures = parse::<Structure>(structures);
    let pools = parse::<TemplatePool>(pools);
    let biomes = Registry::<keys::Biome>::new(keys::BIOME, []).expect("an empty registry");
    let tags = no_tags(&biomes);
    let structure_registry =
        Registry::<keys::Structure>::new(keys::STRUCTURE, []).expect("an empty registry");
    let structure_tags = no_tags(&structure_registry);
    let template = |id: &ResourceLocation| template(id).map(Cow::Owned);
    freeze(&StructureInputs {
        sets: &sets,
        structures: &structures,
        pools: &pools,
        template: &template,
        resolve,
        biomes: &biomes,
        biome_tags: &tags,
        structure_registry: &structure_registry,
        structure_tags: &structure_tags,
        variants: &VariantInputs::default(),
    })
}

fn check(sets: &[(&str, &str)], structures: &[(&str, &str)], pools: &[(&str, &str)]) -> String {
    try_freeze(sets, structures, pools)
        .err()
        .expect("the check fires")
}

const EMPTY_POOL: &str = r#"{"elements": [], "fallback": "minecraft:empty"}"#;

fn jigsaw(extra: &str) -> String {
    jigsaw_reaching(80, extra)
}

fn jigsaw_reaching(max_distance_from_center: i32, extra: &str) -> String {
    format!(
        r#"{{"type": "minecraft:jigsaw", "biomes": [], "spawn_overrides": {{}}, "step": "surface_structures",
            "start_pool": "minecraft:empty", "size": 1, "start_height": {{"absolute": 0}},
            "use_expansion_hack": false, "max_distance_from_center": {max_distance_from_center} {extra}}}"#
    )
}

fn spread(extra: &str) -> String {
    format!(
        r#"{{"structures": [{{"structure": "minecraft:s", "weight": 1}}],
            "placement": {{"type": "minecraft:random_spread", "salt": 1, "spacing": 10, "separation": 2 {extra}}}}}"#
    )
}

#[test]
fn the_jigsaw_range_plus_its_terrain_margin_stays_within_128() {
    let adapted = r#", "terrain_adaptation": "beard_thin""#;
    let at_the_bound = jigsaw_reaching(116, adapted);
    assert!(
        try_freeze(
            &[],
            &[("minecraft:s", &at_the_bound)],
            &[("minecraft:empty", EMPTY_POOL)]
        )
        .is_ok()
    );
}

#[test]
fn every_dangling_id_is_named() {
    assert_eq!(
        check(
            &[("minecraft:a", &spread(""))],
            &[],
            &[("minecraft:empty", EMPTY_POOL)]
        ),
        "minecraft:a: names the structure minecraft:s, which is not loaded"
    );
    assert_eq!(
        check(&[], &[("minecraft:s", &jigsaw(""))], &[]),
        "minecraft:s: names the template pool minecraft:empty, which is not loaded"
    );
    assert_eq!(
        check(&[], &[], &[("minecraft:p", EMPTY_POOL)]),
        "minecraft:p: names the template pool minecraft:empty, which is not loaded"
    );
    let alias = r#", "pool_aliases": [{"type": "minecraft:direct", "alias": "minecraft:x", "target": "minecraft:gone"}]"#;
    assert_eq!(
        check(
            &[],
            &[("minecraft:s", &jigsaw(alias))],
            &[("minecraft:empty", EMPTY_POOL)]
        ),
        "minecraft:s: names the template pool minecraft:gone, which is not loaded"
    );
    let zone = spread(r#", "exclusion_zone": {"other_set": "minecraft:gone", "chunk_count": 1}"#);
    assert_eq!(
        check(
            &[("minecraft:a", &zone)],
            &[("minecraft:s", &jigsaw(""))],
            &[("minecraft:empty", EMPTY_POOL)]
        ),
        "minecraft:a: names the structure set minecraft:gone, which is not loaded"
    );
    let tagged = jigsaw("").replace(
        r#""biomes": []"#,
        r##""biomes": "#minecraft:has_structure/gone""##,
    );
    let error = corpus_set()
        .scope(|| serde_json::from_str::<Structure>(&tagged))
        .expect_err(
            "a biome tag the data pack does not define is refused when the structure is read",
        )
        .to_string();
    assert!(
        error.contains("minecraft:has_structure/gone")
            && error.contains("minecraft:worldgen/biome"),
        "{error}"
    );
    let igloo = r#"{"type": "minecraft:igloo", "biomes": [], "spawn_overrides": {}, "step": "surface_structures"}"#;
    assert_eq!(
        check(&[], &[("minecraft:s", igloo)], &[]),
        "minecraft:s: names the template minecraft:igloo/top, which is not loaded"
    );
}

#[test]
fn a_pool_naming_no_loaded_template_places_an_empty_one() {
    let pool = r#"{"fallback": "minecraft:p", "elements": [{"weight": 2, "element": {
        "element_type": "minecraft:single_pool_element", "location": "minecraft:gone",
        "processors": "minecraft:empty", "projection": "rigid"}}]}"#;
    let frozen = try_freeze(&[], &[], &[("minecraft:p", pool)]).unwrap();
    assert_eq!(frozen.templates.len(), 1);
    assert_eq!(frozen.pools[0].expanded.len(), 2);
    assert_eq!(frozen.pools[0].max_size, 2);
}

#[test]
fn a_list_imposes_its_projection_on_every_member() {
    let pool = r#"{"fallback": "minecraft:p", "elements": [{"weight": 1, "element": {
        "element_type": "minecraft:list_pool_element", "projection": "terrain_matching", "elements": [{
            "element_type": "minecraft:single_pool_element", "location": "minecraft:gone",
            "processors": "minecraft:empty", "projection": "rigid"}]}}]}"#;
    let frozen = try_freeze(&[], &[], &[("minecraft:p", pool)]).unwrap();
    let FrozenElement::List { elements, .. } =
        &frozen.elements[frozen.pools[0].expanded[0].0 as usize]
    else {
        panic!("the pool holds a list");
    };
    assert!(matches!(
        frozen.elements[elements[0].0 as usize],
        FrozenElement::Single {
            projection: Projection::TerrainMatching,
            ..
        }
    ));
}

fn template_with_a_jigsaw_to(pool: &str) -> Template {
    let mut nbt = NbtCompound::new();
    nbt.put("id", "minecraft:jigsaw");
    nbt.put("pool", pool);
    Template {
        size: [1, 1, 1],
        entities: vec![],
        blocks: vec![TemplateBlock {
            nbt: Some(nbt),
            pos: [0, 0, 0],
            state: 0,
        }],
        palette: Some(vec![PaletteState {
            id: rl!("minecraft:jigsaw").to_arc(),
            properties: Some([("orientation".to_owned(), "north_up".to_owned())].into()),
        }]),
        palettes: None,
        data_version: VERSION.world_version,
    }
}

#[test]
fn a_jigsaw_names_a_loaded_pool_or_an_alias_of_its_structure() {
    let pool = r#"{"fallback": "minecraft:empty", "elements": [{"weight": 1, "element": {
        "element_type": "minecraft:single_pool_element", "location": "minecraft:t",
        "processors": "minecraft:empty", "projection": "rigid"}}]}"#;
    let pools = [("minecraft:empty", EMPTY_POOL), ("minecraft:p", pool)];
    let template = |_: &ResourceLocation| Some(template_with_a_jigsaw_to("minecraft:gone"));
    let resolve = |state: &PaletteState| resolve_palette_state(corpus(), state);
    let start = jigsaw("").replace(
        r#""start_pool": "minecraft:empty""#,
        r#""start_pool": "minecraft:p""#,
    );
    assert_eq!(
        try_freeze_with(&[], &[("minecraft:s", &start)], &pools, template, &resolve)
            .err()
            .expect("the check fires"),
        "minecraft:s: the jigsaw at [0, 0, 0] in minecraft:t names the template pool minecraft:gone, which is neither loaded nor aliased"
    );
    let aliased = jigsaw(
        r#", "pool_aliases": [{"type": "minecraft:direct", "alias": "minecraft:gone", "target": "minecraft:empty"}]"#,
    )
    .replace(r#""start_pool": "minecraft:empty""#, r#""start_pool": "minecraft:p""#);
    assert!(
        try_freeze_with(
            &[],
            &[("minecraft:s", &aliased)],
            &pools,
            template,
            &resolve
        )
        .is_ok()
    );
}
