use crate::tree_geometry_parity::{Blocks, compiled, read_dump};
use mcrs_minecraft_worldgen_feature::proto::Feature;

mod exhaustive {
    use super::*;

    #[test]
    fn twenty_features_over_eighty_cases_are_covered() {
        let cases = read_dump();
        let blocks = Blocks::new();
        let corpus: Vec<(String, Feature)> = mcrs_minecraft_worldgen_testing::registry("feature")
            .into_iter()
            .map(|(id, feature)| (id.to_string(), feature))
            .collect();

        let mut covered = std::collections::BTreeSet::new();
        let mut compared = 0usize;
        for case in &cases {
            if compiled(&case.feature, &blocks, &corpus).is_none() {
                continue;
            }
            covered.insert(case.feature.clone());
            compared += 1;
        }

        assert_eq!(
            (covered.len(), compared),
            (20, 80),
            "the covered set shrank; a feature dropped out of the dump or out of the table"
        );
    }
}
