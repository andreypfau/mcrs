use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs::File;
use std::sync::{Arc, LazyLock};

use fixedbitset::FixedBitSet;
use mcrs_minecraft_core::{DynRegistryIndex, DynTagRegistry, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes;
use mcrs_minecraft_world::biome::source::{BiomeSource, MultiNoiseBiomeSource};
use mcrs_minecraft_worldgen::corpus::assets_dir;
use mcrs_minecraft_worldgen::structure::template::{
    PaletteState, ResolvedState, TEMPLATE_DATA_VERSION, Template, TemplateBlock,
};
use mcrs_minecraft_worldgen::structure::{Projection, Structure, StructureSet, TemplatePool};

use super::{biome_index, biome_tags, corpus, load_json_dir};
use crate::world::generate::features::possible_biomes;
use crate::world::generate::structures::{
    FrozenElement, FrozenStructures, StructureInputs, StructureKind, freeze, live_sets,
    resolve_palette_state,
};

/// Every shipped structure whose type has no generator yet; each is frozen as
/// a structure that places nothing.
const HARDCODED: [&str; 24] = [
    "minecraft:buried_treasure",
    "minecraft:desert_pyramid",
    "minecraft:end_city",
    "minecraft:fortress",
    "minecraft:igloo",
    "minecraft:jungle_pyramid",
    "minecraft:mansion",
    "minecraft:mineshaft",
    "minecraft:mineshaft_mesa",
    "minecraft:monument",
    "minecraft:nether_fossil",
    "minecraft:ocean_ruin_cold",
    "minecraft:ocean_ruin_warm",
    "minecraft:ruined_portal",
    "minecraft:ruined_portal_desert",
    "minecraft:ruined_portal_jungle",
    "minecraft:ruined_portal_mountain",
    "minecraft:ruined_portal_nether",
    "minecraft:ruined_portal_ocean",
    "minecraft:ruined_portal_swamp",
    "minecraft:shipwreck",
    "minecraft:shipwreck_beached",
    "minecraft:stronghold",
    "minecraft:swamp_hut",
];

pub(super) fn template_file<'a>(id: &ResourceLocation) -> Option<Cow<'a, Template>> {
    let path = assets_dir()
        .join("minecraft/structure")
        .join(format!("{}.nbt", id.path()));
    let file = File::open(&path).ok()?;
    let template = from_gzip_bytes(file).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Some(Cow::Owned(template))
}

struct Corpus {
    sets: BTreeMap<ResourceLocation, StructureSet>,
    structures: BTreeMap<ResourceLocation, Structure>,
    pools: BTreeMap<ResourceLocation, TemplatePool>,
}

fn corpus_registries() -> Corpus {
    Corpus {
        sets: load_json_dir("structure_set"),
        structures: load_json_dir("structure"),
        pools: load_json_dir("template_pool"),
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
            biomes: biome_index(),
            biome_tags: biome_tags(),
        })
        .unwrap_or_else(|e| panic!("{e}"))
        .into()
    });
    &FROZEN
}

fn structure(id: &str) -> &'static crate::world::generate::structures::FrozenStructure {
    let frozen = frozen();
    let id = ResourceLocation::parse(id).unwrap();
    &frozen.structures[frozen.structure_ids[&id].0 as usize]
}

#[test]
fn the_hardcoded_census_is_pinned() {
    let mut hardcoded: Vec<&str> = frozen()
        .structures
        .iter()
        .filter(|structure| matches!(structure.kind, StructureKind::Hardcoded))
        .map(|structure| structure.id.as_str())
        .collect();
    hardcoded.sort();
    assert_eq!(hardcoded, HARDCODED);
}

#[test]
fn the_corpus_freezes_to_the_pinned_tables() {
    let frozen = frozen();
    assert_eq!(frozen.sets.len(), 21);
    assert_eq!(frozen.structures.len(), 52);
    assert_eq!(frozen.pools.len(), 245);
    assert_eq!(frozen.templates.len(), 1286);
    let missing =
        ResourceLocation::parse("minecraft:ancient_city/walls/intact_horizontal_wall_stairs_5")
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

    let empty = frozen.pool_ids[&ResourceLocation::minecraft("empty")];
    let empty_pool = &frozen.pools[empty.0 as usize];
    assert_eq!(empty_pool.fallback, empty);
    assert!(empty_pool.expanded.is_empty());
    assert_eq!(empty_pool.max_size, 0);
    let houses = &frozen.pools
        [frozen.pool_ids[&ResourceLocation::minecraft("village/plains/houses")].0 as usize];
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
    let mut mask = FixedBitSet::with_capacity(biome_index().len() as usize);
    for id in possible_biomes(source, |_| None) {
        mask.insert(biome_index().get(id.as_str()).unwrap() as usize);
    }
    live_sets(frozen, &mask)
        .into_iter()
        .map(|(set, _)| frozen.sets[set.0 as usize].id.to_string())
        .collect()
}

pub(super) fn preset(name: &str) -> BiomeSource {
    BiomeSource::MultiNoise(MultiNoiseBiomeSource {
        preset: Some(ResourceLocation::parse(name).unwrap()),
        biomes: None,
    })
}

#[test]
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
                ResourceLocation::parse(id).unwrap(),
                serde_json::from_str(json).unwrap_or_else(|e| panic!("{id}: {e}")),
            )
        })
        .collect()
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
    let biomes = DynRegistryIndex::build(std::iter::empty());
    let tags = DynTagRegistry::default();
    let template = |id: &ResourceLocation| template(id).map(Cow::Owned);
    freeze(&StructureInputs {
        sets: &sets,
        structures: &structures,
        pools: &pools,
        template: &template,
        resolve,
        biomes: &biomes,
        biome_tags: &tags,
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
fn spacing_must_exceed_separation() {
    let set = r#"{"structures": [{"structure": "minecraft:s", "weight": 1}],
        "placement": {"type": "minecraft:random_spread", "salt": 1, "spacing": 8, "separation": 8}}"#;
    assert_eq!(
        check(
            &[("minecraft:a", set)],
            &[("minecraft:s", &jigsaw(""))],
            &[("minecraft:empty", EMPTY_POOL)]
        ),
        "minecraft:a: spacing 8 is not larger than separation 8"
    );
}

#[test]
fn exclusion_zones_must_not_cycle() {
    let a = spread(r#", "exclusion_zone": {"other_set": "minecraft:b", "chunk_count": 1}"#);
    let b = spread(r#", "exclusion_zone": {"other_set": "minecraft:a", "chunk_count": 1}"#);
    assert_eq!(
        check(
            &[("minecraft:a", &a), ("minecraft:b", &b)],
            &[("minecraft:s", &jigsaw(""))],
            &[("minecraft:empty", EMPTY_POOL)]
        ),
        "minecraft:a: the exclusion zones cycle: minecraft:a -> minecraft:b -> minecraft:a"
    );
}

#[test]
fn an_alias_binds_once() {
    let aliases = r#", "pool_aliases": [
        {"type": "minecraft:direct", "alias": "minecraft:x", "target": "minecraft:empty"},
        {"type": "minecraft:random_group", "groups": [{"weight": 1, "data": [
            {"type": "minecraft:random", "alias": "minecraft:x", "targets": [{"weight": 1, "data": "minecraft:empty"}]}
        ]}]}]"#;
    assert_eq!(
        check(
            &[],
            &[("minecraft:s", &jigsaw(aliases))],
            &[("minecraft:empty", EMPTY_POOL)]
        ),
        "minecraft:s: the pool alias minecraft:x is bound twice"
    );
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
    let structure = jigsaw_reaching(120, adapted);
    assert_eq!(
        check(
            &[],
            &[("minecraft:s", &structure)],
            &[("minecraft:empty", EMPTY_POOL)]
        ),
        "minecraft:s: max_distance_from_center 120 plus the terrain adaptation margin 12 exceeds 128"
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
    assert_eq!(
        check(
            &[],
            &[("minecraft:s", &tagged)],
            &[("minecraft:empty", EMPTY_POOL)]
        ),
        "minecraft:s: names the biome tag #minecraft:has_structure/gone, which is not loaded"
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
            id: ResourceLocation::minecraft("jigsaw"),
            properties: Some([("orientation".to_owned(), "north_up".to_owned())].into()),
        }]),
        palettes: None,
        data_version: TEMPLATE_DATA_VERSION,
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
