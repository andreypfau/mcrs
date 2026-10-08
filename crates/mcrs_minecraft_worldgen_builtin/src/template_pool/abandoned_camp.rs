use mcrs_minecraft_biome_file::PlacedFeatureKey;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_feature::keys::placed_feature;
use mcrs_minecraft_worldgen_feature::pool::{PoolElement, SingleElement, TemplatePool};
use mcrs_minecraft_worldgen_feature::template::Projection::Rigid;

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

const TREES: [(&str, PlacedFeatureKey); 21] = [
    ("acacia", placed_feature::ACACIA_CHECKED),
    ("birch", placed_feature::BIRCH_CHECKED),
    ("fancy_oak", placed_feature::FANCY_OAK_CHECKED),
    ("oak", placed_feature::OAK_CHECKED),
    ("spruce", placed_feature::SPRUCE_CHECKED),
    ("thick_spruce", placed_feature::MEGA_SPRUCE_CHECKED),
    ("yellow_poplar", placed_feature::YELLOW_POPLAR),
    ("orange_poplar", placed_feature::ORANGE_POPLAR),
    ("red_poplar", placed_feature::RED_POPLAR),
    ("super_birch_bees", placed_feature::SUPER_BIRCH_BEES_0002),
    ("spruce_on_snow", placed_feature::SPRUCE_ON_SNOW),
    ("fancy_oak_bees", placed_feature::FANCY_OAK_BEES_002),
    ("birch_bees", placed_feature::BIRCH_BEES_002),
    ("pale_oak", placed_feature::PALE_OAK_CHECKED),
    ("bamboo", placed_feature::BAMBOO_IN_STRUCTURE),
    ("jungle", placed_feature::JUNGLE_TREE),
    ("pine", placed_feature::PINE_CHECKED),
    ("mega_pine", placed_feature::MEGA_PINE_CHECKED),
    ("mega_jungle", placed_feature::MEGA_JUNGLE_TREE_CHECKED),
    ("cherry", placed_feature::CHERRY_CHECKED),
    ("cherry_bees", placed_feature::CHERRY_BEES_005),
];

const DEFAULT_CAMP_TYPES: [&str; 3] = ["chest", "barrel", "special"];
const NUM_OF_BIOME_SPECIFIC_CAMPSITES: i32 = 4;
const NUM_OF_DEFAULT_CAMPSITES: i32 = 15;
const NUM_OF_TENTS: i32 = 10;

fn pool(elements: impl IntoIterator<Item = PoolElement>) -> TemplatePool {
    super::entries_pool("empty", elements.into_iter().map(|element| (element, 1)))
}

fn legacy(location: String) -> PoolElement {
    let location = ResourceLocation::minecraft(&location).expect("a hardcoded name");
    PoolElement::LegacySingle(SingleElement::new(location, None, Rigid))
}

fn tree(feature: PlacedFeatureKey) -> TemplatePool {
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
        .map(|name| ResourceLocation::minecraft(&name).expect("a hardcoded name"))
}

pub fn all() -> impl Iterator<Item = (ResourceLocation, TemplatePool)> {
    keys().map(|id| {
        let pool = build(&id).expect("a listed camp pool builds");
        (id, pool)
    })
}

fn build(id: &ResourceLocation) -> Option<TemplatePool> {
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
