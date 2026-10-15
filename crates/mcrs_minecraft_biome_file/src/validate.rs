use crate::BiomeGenerationSettings;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_worldgen_feature::keys::PLACED_FEATURE;
use mcrs_minecraft_worldgen_feature::proto::PlacedFeature;
use std::collections::HashSet;

const ALLOWED_MIN: i32 = -16;
const ALLOWED_MAX: i32 = 31;

/// One failure per biome whose placed features reach outside the horizontal
/// range a feature may cover. A placed feature is checked under the first
/// biome that names it; a later biome naming it again still counts it in its
/// feature index.
pub fn check_feature_domains(
    settings: &[BiomeGenerationSettings],
    set: &RegistrySet,
) -> Vec<(usize, String)> {
    let features = set
        .column::<PlacedFeature>(PLACED_FEATURE.location().as_static_str())
        .expect("the loader parses the placed features biomes name");
    let names = set
        .registry::<PlacedFeature>()
        .expect("the set holds the placed feature registry");
    let tags = set
        .tags::<PlacedFeature>()
        .expect("the set holds the placed feature tags");

    let mut visited = HashSet::new();
    let mut failures = Vec::new();
    for (biome, generation) in settings.iter().enumerate() {
        let mut too_wide = Vec::new();
        for (step, features_of_step) in generation.features.iter().enumerate() {
            for (index, feature) in features_of_step.ids(&tags).enumerate() {
                if !visited.insert(feature.index()) {
                    continue;
                }
                let domain = features[feature.index()].xz_domain();
                let (min, max) = (*domain.start(), *domain.end());
                if min < ALLOWED_MIN || max > ALLOWED_MAX {
                    let name = names.name(feature).expect("a placed feature has a name");
                    too_wide.push(format!(
                        "{name} (features[{step}][{index}]) has [{min}, {max}]"
                    ));
                }
            }
        }
        if !too_wide.is_empty() {
            failures.push((
                biome,
                format!(
                    "Placement(s) cover too large domain in XZ plane, must be at most [{ALLOWED_MIN}, {ALLOWED_MAX}]: {}",
                    too_wide.join(", ")
                ),
            ));
        }
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CarverSet;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_registry::static_report::shipped_report;
    use mcrs_minecraft_registry::{HolderSet, NameTable, TagTable};
    use mcrs_minecraft_worldgen_feature::keys::FEATURE;
    use std::sync::Arc;

    fn square_chain(squares: usize) -> String {
        let square = r#"{"type":"minecraft:in_square"}"#;
        let chain = vec![square; squares].join(",");
        format!(r#"{{"feature":"minecraft:f","placement":[{chain}]}}"#)
    }

    fn loaded() -> RegistrySet {
        let table = |key: &str, names: &[&str]| {
            let names = names.iter().map(|name| name.parse().unwrap());
            Arc::new(NameTable::new(key.parse().unwrap(), names).unwrap())
        };
        let placed = PLACED_FEATURE.location().as_static_str();
        let placed_table = table(
            placed,
            &["minecraft:fine", "minecraft:wide", "minecraft:wider"],
        );
        let no_tags: [(ResourceLocation<Arc<str>>, [u16; 0]); 0] = [];
        let set = shipped_report()
            .clone()
            .with_types(mcrs_minecraft_registry_catalog::bindings())
            .unwrap()
            .with_tables(vec![
                table(FEATURE.location().as_static_str(), &["minecraft:f"]),
                Arc::clone(&placed_table),
            ])
            .unwrap()
            .with_tags(Arc::new(
                TagTable::numbered(&placed_table, no_tags).unwrap(),
            ));
        let values: Arc<[PlacedFeature]> = set.scope(|| {
            [1, 3, 4]
                .map(|squares| serde_json::from_str(&square_chain(squares)).unwrap())
                .into()
        });
        set.with_column(placed, values).unwrap()
    }

    #[test]
    fn each_feature_is_reported_once_under_the_first_biome_naming_it() {
        let set = loaded();
        let features = set.registry::<PlacedFeature>().unwrap();
        let named = |names: &[&str]| {
            HolderSet::List(
                names
                    .iter()
                    .map(|name| features.by_name(name).unwrap())
                    .collect(),
            )
        };
        let biome = |steps: Vec<HolderSet<PlacedFeature>>| BiomeGenerationSettings {
            carvers: CarverSet::default(),
            features: steps,
        };
        let settings = [
            biome(vec![named(&["minecraft:fine", "minecraft:wide"])]),
            biome(vec![named(&["minecraft:wide", "minecraft:wider"])]),
            biome(vec![named(&["minecraft:fine"]), named(&["minecraft:wide"])]),
        ];

        let failures = set.scope(|| check_feature_domains(&settings, &set));
        let reported: Vec<(usize, &str)> = failures
            .iter()
            .map(|(biome, message)| {
                let (_, features) = message
                    .split_once("[-16, 31]: ")
                    .expect("the message states the allowed range");
                (*biome, features)
            })
            .collect();
        assert_eq!(
            reported,
            [
                (0, "minecraft:wide (features[0][1]) has [0, 45]"),
                (1, "minecraft:wider (features[0][1]) has [0, 60]"),
            ]
        );
    }
}
