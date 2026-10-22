use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_feature::proto::PlacedFeature;
use std::collections::HashSet;

const ALLOWED_MIN: i32 = -16;
const ALLOWED_MAX: i32 = 31;

/// One failure per biome whose placed features reach outside the horizontal
/// range a feature may cover. Each biome lists its steps, each step the named
/// placed features it runs. A placed feature is checked under the first biome
/// that names it; a later biome naming it again still counts it in its
/// feature index.
pub fn check_feature_domains<'a>(
    biomes: impl IntoIterator<Item = Vec<Vec<(&'a ResourceLocation, &'a PlacedFeature)>>>,
) -> Vec<(usize, String)> {
    let mut visited = HashSet::new();
    let mut failures = Vec::new();
    for (biome, steps) in biomes.into_iter().enumerate() {
        let mut too_wide = Vec::new();
        for (step, features_of_step) in steps.iter().enumerate() {
            for (index, &(name, feature)) in features_of_step.iter().enumerate() {
                if !visited.insert(name) {
                    continue;
                }
                let domain = feature.xz_domain();
                let (min, max) = (*domain.start(), *domain.end());
                if min < ALLOWED_MIN || max > ALLOWED_MAX {
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

    fn square_chain(squares: usize) -> PlacedFeature {
        let square = r#"{"type":"minecraft:in_square"}"#;
        let chain = vec![square; squares].join(",");
        serde_json::from_str(&format!(
            r#"{{"feature":"minecraft:f","placement":[{chain}]}}"#
        ))
        .unwrap()
    }

    #[test]
    fn each_feature_is_reported_once_under_the_first_biome_naming_it() {
        let names = ["minecraft:fine", "minecraft:wide", "minecraft:wider"]
            .map(|name| ResourceLocation::read(name).unwrap());
        let features = [1, 3, 4].map(square_chain);
        let named = |picked: &[usize]| -> Vec<_> {
            picked
                .iter()
                .map(|&at| (&names[at], &features[at]))
                .collect()
        };
        let biomes = [
            vec![named(&[0, 1])],
            vec![named(&[1, 2])],
            vec![named(&[0]), named(&[1])],
        ];

        let failures = check_feature_domains(biomes);
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
