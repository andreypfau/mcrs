use crate::keys::{PlacedKey, placed};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_feature::template::Projection::Rigid;
use mcrs_minecraft_worldgen_structure::{PoolElement, SingleElement, TemplatePool};

const BIOME_VARIANTS: [&str; 18] = [
    "savanna",
    "flower_forest",
    "birch_forest",
    "forest",
    "snowy_taiga",
    "bamboo_jungle",
    "sparse_jungle",
    "cherry_grove",
    "meadow",
    "old_growth_birch_forest",
    "old_growth_spruce_taiga",
    "old_growth_pine_taiga",
    "swamp",
    "taiga",
    "windswept_forest",
    "dappled_forest",
    "wooded_badlands",
    "pale_garden",
];

const TREES: [(&str, PlacedKey); 21] = [
    ("acacia", placed::ACACIA_CHECKED),
    ("birch", placed::BIRCH_CHECKED),
    ("fancy_oak", placed::FANCY_OAK_CHECKED),
    ("oak", placed::OAK_CHECKED),
    ("spruce", placed::SPRUCE_CHECKED),
    ("thick_spruce", placed::MEGA_SPRUCE_CHECKED),
    ("yellow_poplar", placed::YELLOW_POPLAR),
    ("orange_poplar", placed::ORANGE_POPLAR),
    ("red_poplar", placed::RED_POPLAR),
    ("super_birch_bees", placed::SUPER_BIRCH_BEES_0002),
    ("spruce_on_snow", placed::SPRUCE_ON_SNOW),
    ("fancy_oak_bees", placed::FANCY_OAK_BEES_002),
    ("birch_bees", placed::BIRCH_BEES_002),
    ("pale_oak", placed::PALE_OAK_CHECKED),
    ("bamboo", placed::BAMBOO_IN_STRUCTURE),
    ("jungle", placed::JUNGLE_TREE),
    ("pine", placed::PINE_CHECKED),
    ("mega_pine", placed::MEGA_PINE_CHECKED),
    ("mega_jungle", placed::MEGA_JUNGLE_TREE_CHECKED),
    ("cherry", placed::CHERRY_CHECKED),
    ("cherry_bees", placed::CHERRY_BEES_005),
];

const DEFAULT_CAMP_TYPES: [&str; 3] = ["chest", "barrel", "special"];
const NUM_OF_BIOME_SPECIFIC_CAMPSITES: i32 = 4;
const NUM_OF_DEFAULT_CAMPSITES: i32 = 15;
const NUM_OF_TENTS: i32 = 10;

fn pool(elements: impl IntoIterator<Item = PoolElement>) -> TemplatePool {
    super::entries_pool("empty", elements.into_iter().map(|element| (element, 1)))
}

fn legacy(location: String) -> PoolElement {
    let location = ResourceLocation::minecraft(&location);
    PoolElement::LegacySingle(SingleElement::new(location, None, Rigid))
}

fn tree(feature: PlacedKey) -> TemplatePool {
    pool([PoolElement::Feature {
        feature: feature.into(),
        projection: Rigid,
    }])
}

fn tents(biome: &str) -> TemplatePool {
    pool(
        (1..=NUM_OF_TENTS).map(|n| legacy(format!("abandoned_camp/tent/{biome}/tent_{biome}_{n}"))),
    )
}

fn campsites(biome: &str) -> TemplatePool {
    let default_camps = DEFAULT_CAMP_TYPES.iter().flat_map(|kind| {
        (1..=NUM_OF_DEFAULT_CAMPSITES).map(move |n| {
            legacy(format!(
                "abandoned_camp/camp/default/campsite_default_{kind}_{n}"
            ))
        })
    });
    let biome_camps = (1..=NUM_OF_BIOME_SPECIFIC_CAMPSITES)
        .map(|n| legacy(format!("abandoned_camp/camp/{biome}/campsite_{biome}_{n}")));
    pool(default_camps.chain(biome_camps))
}

pub fn keys() -> impl Iterator<Item = ResourceLocation> {
    let trees = TREES
        .iter()
        .map(|(name, _)| format!("abandoned_camp/trees/{name}"));
    let campsites = BIOME_VARIANTS.iter().flat_map(|biome| {
        [
            format!("abandoned_camp/tent/{biome}"),
            format!("abandoned_camp/camp/{biome}"),
        ]
    });
    trees
        .chain(campsites)
        .map(|name| ResourceLocation::minecraft(&name))
}

pub fn all() -> impl Iterator<Item = (ResourceLocation, TemplatePool)> {
    keys().map(|id| {
        let pool = build(&id).expect("a listed camp pool builds");
        (id, pool)
    })
}

pub fn build(id: &ResourceLocation) -> Option<TemplatePool> {
    let name = id.path().strip_prefix("abandoned_camp/")?;
    if id.namespace() != "minecraft" {
        return None;
    }
    let (kind, name) = name.split_once('/')?;
    let biome = || BIOME_VARIANTS.iter().find(|biome| **biome == name);
    match kind {
        "trees" => {
            let (_, feature) = TREES.iter().find(|(tree, _)| *tree == name)?;
            Some(tree(*feature))
        }
        "tent" => biome().map(|biome| tents(biome)),
        "camp" => biome().map(|biome| campsites(biome)),
        _ => None,
    }
}
