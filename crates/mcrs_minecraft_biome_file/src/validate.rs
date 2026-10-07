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
    use mcrs_minecraft_registry::static_report::shipped_report;
    use mcrs_minecraft_registry::{HolderSet, Pack, PackFile, WorldRegistries};
    use mcrs_minecraft_worldgen_feature::keys::FEATURE;

    fn square_chain(squares: usize) -> String {
        let square = r#"{"type":"minecraft:in_square"}"#;
        let chain = vec![square; squares].join(",");
        format!(r#"{{"feature":"minecraft:f","placement":[{chain}]}}"#)
    }

    fn loaded() -> RegistrySet {
        let statics = shipped_report()
            .clone()
            .with_types(mcrs_minecraft_registry_catalog::bindings())
            .unwrap();
        let mut registries =
            WorldRegistries::new([FEATURE.location(), PLACED_FEATURE.location()].map(Into::into));
        registries.parse::<PlacedFeature>(PLACED_FEATURE.location());
        let file = |path: &str, json: Option<String>| PackFile {
            path: path.to_owned(),
            bytes: json.map(String::into_bytes),
        };
        let pack = Pack {
            name: "test".to_owned(),
            files: vec![
                file("minecraft/worldgen/feature/f.json", None),
                file(
                    "minecraft/worldgen/placed_feature/fine.json",
                    Some(square_chain(1)),
                ),
                file(
                    "minecraft/worldgen/placed_feature/wide.json",
                    Some(square_chain(3)),
                ),
                file(
                    "minecraft/worldgen/placed_feature/wider.json",
                    Some(square_chain(4)),
                ),
            ],
            built: Vec::new(),
        };
        registries
            .load(&statics, &[pack])
            .unwrap_or_else(|report| panic!("{report}"))
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
