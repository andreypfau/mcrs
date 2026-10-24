use mcrs_minecraft_biome::keys::placed_feature;
use mcrs_minecraft_biome_file::PlacedFeatureKey;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::resource_location::InvalidResourceLocation;
use mcrs_minecraft_core::{ResourceLocation, StaticResourceLocation, rl};
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

#[rustfmt::skip]
const TREES: [(StaticResourceLocation, PlacedFeatureKey); 21] = [
    (rl!("minecraft:abandoned_camp/trees/acacia"), placed_feature::ACACIA_CHECKED),
    (rl!("minecraft:abandoned_camp/trees/birch"), placed_feature::BIRCH_CHECKED),
    (rl!("minecraft:abandoned_camp/trees/fancy_oak"), placed_feature::FANCY_OAK_CHECKED),
    (rl!("minecraft:abandoned_camp/trees/oak"), placed_feature::OAK_CHECKED),
    (rl!("minecraft:abandoned_camp/trees/spruce"), placed_feature::SPRUCE_CHECKED),
    (rl!("minecraft:abandoned_camp/trees/thick_spruce"), placed_feature::MEGA_SPRUCE_CHECKED),
    (rl!("minecraft:abandoned_camp/trees/yellow_poplar"), placed_feature::YELLOW_POPLAR),
    (rl!("minecraft:abandoned_camp/trees/orange_poplar"), placed_feature::ORANGE_POPLAR),
    (rl!("minecraft:abandoned_camp/trees/red_poplar"), placed_feature::RED_POPLAR),
    (rl!("minecraft:abandoned_camp/trees/super_birch_bees"), placed_feature::SUPER_BIRCH_BEES_0002),
    (rl!("minecraft:abandoned_camp/trees/spruce_on_snow"), placed_feature::SPRUCE_ON_SNOW),
    (rl!("minecraft:abandoned_camp/trees/fancy_oak_bees"), placed_feature::FANCY_OAK_BEES_002),
    (rl!("minecraft:abandoned_camp/trees/birch_bees"), placed_feature::BIRCH_BEES_002),
    (rl!("minecraft:abandoned_camp/trees/pale_oak"), placed_feature::PALE_OAK_CHECKED),
    (rl!("minecraft:abandoned_camp/trees/bamboo"), placed_feature::BAMBOO_IN_STRUCTURE),
    (rl!("minecraft:abandoned_camp/trees/jungle"), placed_feature::JUNGLE_TREE),
    (rl!("minecraft:abandoned_camp/trees/pine"), placed_feature::PINE_CHECKED),
    (rl!("minecraft:abandoned_camp/trees/mega_pine"), placed_feature::MEGA_PINE_CHECKED),
    (rl!("minecraft:abandoned_camp/trees/mega_jungle"), placed_feature::MEGA_JUNGLE_TREE_CHECKED),
    (rl!("minecraft:abandoned_camp/trees/cherry"), placed_feature::CHERRY_CHECKED),
    (rl!("minecraft:abandoned_camp/trees/cherry_bees"), placed_feature::CHERRY_BEES_005),
];

const DEFAULT_CAMP_TYPES: [&str; 3] = ["chest", "barrel", "special"];
const NUM_OF_BIOME_SPECIFIC_CAMPSITES: i32 = 4;
const NUM_OF_DEFAULT_CAMPSITES: i32 = 15;
const NUM_OF_TENTS: i32 = 10;

fn pool(elements: impl IntoIterator<Item = PoolElement>) -> TemplatePool {
    let elements = elements.into_iter().map(|element| (element, Bounded(1)));
    super::entries_pool(rl!("minecraft:empty"), elements)
}

fn legacy(location: String) -> Result<PoolElement, InvalidResourceLocation> {
    let location = ResourceLocation::minecraft(&location)?;
    Ok(PoolElement::LegacySingle(SingleElement::new(
        location, None, Rigid,
    )))
}

fn tree(feature: PlacedFeatureKey) -> TemplatePool {
    pool([PoolElement::Feature {
        feature: feature.into(),
        projection: Rigid,
    }])
}

fn tents(biome: &str) -> Result<TemplatePool, InvalidResourceLocation> {
    (1..=NUM_OF_TENTS)
        .map(|n| legacy(format!("abandoned_camp/tent/{biome}/tent_{biome}_{n}")))
        .collect::<Result<Vec<_>, _>>()
        .map(pool)
}

fn campsites(biome: &str) -> Result<TemplatePool, InvalidResourceLocation> {
    let default_camps = DEFAULT_CAMP_TYPES.iter().flat_map(|kind| {
        (1..=NUM_OF_DEFAULT_CAMPSITES).map(move |n| {
            legacy(format!(
                "abandoned_camp/camp/default/campsite_default_{kind}_{n}"
            ))
        })
    });
    let biome_camps = (1..=NUM_OF_BIOME_SPECIFIC_CAMPSITES)
        .map(|n| legacy(format!("abandoned_camp/camp/{biome}/campsite_{biome}_{n}")));
    default_camps
        .chain(biome_camps)
        .collect::<Result<Vec<_>, _>>()
        .map(pool)
}

pub fn all()
-> impl Iterator<Item = Result<(ResourceLocation, TemplatePool), InvalidResourceLocation>> {
    let trees = TREES
        .iter()
        .map(|(name, feature)| Ok(((*name).into(), tree(*feature))));
    let camps = BIOME_VARIANTS.iter().flat_map(|biome| {
        [
            ResourceLocation::minecraft(&format!("abandoned_camp/tent/{biome}"))
                .and_then(|name| Ok((name, tents(biome)?))),
            ResourceLocation::minecraft(&format!("abandoned_camp/camp/{biome}"))
                .and_then(|name| Ok((name, campsites(biome)?))),
        ]
    });
    trees.chain(camps)
}
