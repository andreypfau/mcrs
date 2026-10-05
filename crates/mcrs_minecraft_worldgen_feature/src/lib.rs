pub mod block_predicate;
pub mod compile;
pub mod placement;
pub mod placer;
pub mod proto;
pub mod rule_test;
pub mod sort;
pub mod spawn_condition;
pub mod tree;

pub mod column;
pub mod template;

#[cfg(test)]
mod tests {
    use crate::block_predicate::BlockPredicate;
    use crate::placement::PlacementModifier;
    use crate::proto::{Feature, FeatureStepList, PlacedFeature, StructureProcessorList};
    use crate::spawn_condition::{DoubleBounds, SpawnSelector};
    use crate::tree::{BlockStateProvider, FeatureSize, TrunkPlacer};
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    type Codec = fn(&str) -> Result<String, String>;

    fn codec<T: DeserializeOwned + Serialize>(json: &str) -> Result<String, String> {
        let value: T = serde_json::from_str(json).map_err(|error| error.to_string())?;
        Ok(serde_json::to_string(&value).unwrap())
    }

    /// Shapes the shipped corpus never writes, so the corpus round trip does not
    /// cover them.
    #[test]
    fn the_unshipped_shapes_round_trip() {
        let cases: &[(Codec, &str)] = &[
            (
                codec::<PlacedFeature>,
                r#"{"feature":{"type":"minecraft:no_op"},"placement":[]}"#,
            ),
            (
                codec::<PlacedFeature>,
                r#"{"feature":"minecraft:oak","placement":[]}"#,
            ),
            (
                codec::<FeatureStepList>,
                r##""#minecraft:has_structure/village""##,
            ),
            (codec::<FeatureStepList>, r#"["minecraft:oak"]"#),
            (
                codec::<FeatureStepList>,
                r#"[{"feature":{"type":"minecraft:no_op"},"placement":[{"type":"minecraft:count","count":1}]}]"#,
            ),
            (
                codec::<Feature>,
                r#"{"type":"minecraft:fill_layer","height":8,"state":"minecraft:lava"}"#,
            ),
            (
                codec::<Feature>,
                r#"{"type":"minecraft:replace_single_block","targets":[{"target":{"predicate_type":"minecraft:blockstate_match","block_state":"minecraft:stone"},"state":"minecraft:emerald_block"}]}"#,
            ),
            (
                codec::<StructureProcessorList>,
                r#"[{"processor_type":"minecraft:nop"}]"#,
            ),
            (
                codec::<StructureProcessorList>,
                r#"{"processors":[{"processor_type":"minecraft:nop"}]}"#,
            ),
            (
                codec::<TrunkPlacer>,
                r#"{"type":"minecraft:straight_trunk_placer","base_height":5,"height_rand_a":2,"height_rand_b":0}"#,
            ),
            (
                codec::<TrunkPlacer>,
                r#"{"type":"minecraft:straight_trunk_placer","base_height":5,"height_rand_a":2,"height_rand_b":0,"trunk_width":{"type":"minecraft:uniform","min_inclusive":1,"max_inclusive":2}}"#,
            ),
            (
                codec::<BlockStateProvider>,
                r#"{"type":"minecraft:copy_properties","source":{"type":"minecraft:simple","state":"minecraft:oak_log"}}"#,
            ),
            (
                codec::<BlockStateProvider>,
                r#""minecraft:soil_beneath_tree""#,
            ),
            (codec::<BlockStateProvider>, r#"{"id":"minecraft:clay"}"#),
            (
                codec::<BlockStateProvider>,
                r#"{"id":"minecraft:acacia_log","properties":{"axis":"y"}}"#,
            ),
            (
                codec::<FeatureSize>,
                r#"{"type":"minecraft:two_layers_feature_size","min_clipped_height":4}"#,
            ),
            (
                codec::<PlacementModifier>,
                r#"{"type":"minecraft:fixed_placement","positions":[[1,2,3]]}"#,
            ),
            (codec::<DoubleBounds>, "0.9"),
            (codec::<DoubleBounds>, r#"{"min":0.9}"#),
            (codec::<DoubleBounds>, r#"{"min":0.1,"max":0.5}"#),
            (codec::<DoubleBounds>, "{}"),
            (codec::<SpawnSelector>, r#"{"priority":0}"#),
            (
                codec::<SpawnSelector>,
                r##"{"condition":{"type":"minecraft:structure","structures":"#minecraft:cats_spawn_as_black"},"priority":1}"##,
            ),
        ];
        for (codec, json) in cases {
            assert_eq!(codec(json).as_deref(), Ok(*json));
        }
    }

    #[test]
    fn a_malformed_shape_is_a_load_error() {
        let cases: &[(Codec, &str, &str)] = &[
            (
                codec::<BlockPredicate>,
                r#"{"type":"minecraft:volume_match","min":[0,0,0],"max":[0,-1,0],"match":{"type":"minecraft:true"}}"#,
                "min bound cannot be larger",
            ),
            (
                codec::<Feature>,
                r#"{"type":"minecraft:ore","targets":[],"size":65,"discard_chance_on_air_exposure":0.0}"#,
                "[0;64]",
            ),
            (
                codec::<FeatureSize>,
                r#"{"type":"minecraft:two_layers_feature_size","upper_size":17}"#,
                "[0;16]: 17",
            ),
            (
                codec::<FeatureStepList>,
                r#""minecraft:oak""#,
                "Not a tag id",
            ),
            (
                codec::<PlacementModifier>,
                r#"{"type":"minecraft:fixed_placement","positions":[]}"#,
                "",
            ),
            (
                codec::<TrunkPlacer>,
                r#"{"type":"minecraft:straight_trunk_placer","base_height":5,"height_rand_a":2,"height_rand_b":0,"trunk_width":0}"#,
                "",
            ),
            (codec::<DoubleBounds>, r#"{"min":2,"max":1}"#, ""),
        ];
        for (codec, json, message) in cases {
            let error = codec(json).expect_err(json);
            assert!(error.contains(message), "{json}: {error}");
        }
    }
}
