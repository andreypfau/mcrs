use std::collections::BTreeMap;

use serde::Deserialize;
use serde::de::{self, Deserializer};

use super::schema::BoxList;
use mcrs_minecraft_core::direction::{Direction, DirectionSet};
use mcrs_minecraft_registries::TagEntry;
use mcrs_minecraft_resource::ResourceLocationBuf;

/// `minecraft:placement_filter`: the support predicate of a state, in the short form of Bedrock
/// or in the predicate form, with the directions from which a shape update tests it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacementFilter {
    pub rule: PlacementRule,
    pub tested_directions: Option<DirectionSet>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlacementRule {
    /// The block stands when, for some condition, the neighbour on one of its `allowed_faces` is
    /// in `block_filter`. A face names the side of the neighbour the block is placed against, so
    /// `up` is the block below.
    Conditions(Vec<PlacementCondition>),
    Predicate(SupportPredicate),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementCondition {
    pub allowed_faces: Vec<Direction>,
    pub block_filter: Vec<TagEntry>,
}

/// A block predicate of the data pack, restricted to the types and the offsets a shape update
/// can decide.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum SupportPredicate {
    #[serde(rename = "minecraft:matching_blocks", alias = "matching_blocks")]
    MatchingBlocks {
        #[serde(default)]
        offset: Offset,
        #[serde(deserialize_with = "one_or_many")]
        blocks: Vec<TagEntry>,
        #[serde(default)]
        state: StateMatch,
    },
    #[serde(rename = "minecraft:matching_block_tag", alias = "matching_block_tag")]
    MatchingBlockTag {
        #[serde(default)]
        offset: Offset,
        tag: ResourceLocationBuf,
    },
    // chisle: `state` of a fluid is refused at load; no support predicate reads one yet, and the
    // fluid properties of the data pack mapped onto the corpus fluid state lift it.
    #[serde(rename = "minecraft:matching_fluids", alias = "matching_fluids")]
    MatchingFluids {
        #[serde(default)]
        offset: Offset,
        #[serde(deserialize_with = "one_or_many")]
        fluids: Vec<TagEntry>,
    },
    #[serde(rename = "minecraft:has_sturdy_face", alias = "has_sturdy_face")]
    HasSturdyFace {
        #[serde(default)]
        offset: Offset,
        direction: Direction,
        #[serde(default)]
        face_test: FaceTest,
        #[serde(default)]
        shape: FaceShape,
    },
    #[serde(rename = "minecraft:solid", alias = "solid")]
    Solid {
        #[serde(default)]
        offset: Offset,
    },
    #[serde(rename = "minecraft:replaceable", alias = "replaceable")]
    Replaceable {
        #[serde(default)]
        offset: Offset,
    },
    #[serde(rename = "minecraft:not", alias = "not")]
    Not { predicate: Box<SupportPredicate> },
    #[serde(rename = "minecraft:any_of", alias = "any_of")]
    AnyOf { predicates: Vec<SupportPredicate> },
    #[serde(rename = "minecraft:all_of", alias = "all_of")]
    AllOf { predicates: Vec<SupportPredicate> },
    #[serde(rename = "minecraft:true", alias = "true")]
    True,
}

/// The position a leaf reads: P itself, or the neighbour one step towards a direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Offset(pub Option<Direction>);

impl<'de> Deserialize<'de> for Offset {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let axes = <[i32; 3]>::deserialize(d)?;
        if axes == [0, 0, 0] {
            return Ok(Offset(None));
        }
        Direction::all()
            .into_iter()
            .find(|direction| direction.normal().to_array() == axes)
            .map(|direction| Offset(Some(direction)))
            .ok_or_else(|| {
                de::Error::custom(format!(
                    "offset {axes:?} is not P or one of its six neighbours"
                ))
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaceTest {
    #[default]
    Full,
    Center,
    Ring,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaceShape {
    #[default]
    Support,
    Collision,
}

/// The `state` field of a data pack predicate: each named property has one value, or a value
/// within a range.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(transparent)]
pub struct StateMatch(pub BTreeMap<String, ValueMatch>);

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum ValueMatch {
    Exact(String),
    Range {
        #[serde(default)]
        min: Option<String>,
        #[serde(default)]
        max: Option<String>,
    },
}

/// `minecraft:support`: the boxes whose faces the face tests read.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Support {
    pub boxes: BoxList,
}

fn one_or_many<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<TagEntry>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        One(String),
        Many(Vec<String>),
    }
    let names = match Repr::deserialize(d)? {
        Repr::One(name) => vec![name],
        Repr::Many(names) => names,
    };
    names.iter().map(|name| entry(name)).collect()
}

fn entry<E: de::Error>(text: &str) -> Result<TagEntry, E> {
    let (tag, name) = match text.strip_prefix('#') {
        Some(name) => (true, name),
        None => (false, text),
    };
    let id = name.parse::<ResourceLocationBuf>().map_err(E::custom)?;
    Ok(TagEntry {
        id,
        tag,
        required: true,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FilterRepr {
    conditions: Option<Vec<ConditionRepr>>,
    predicate: Option<SupportPredicate>,
    tested_directions: Option<TestedRepr>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConditionRepr {
    allowed_faces: Option<Vec<Direction>>,
    block_filter: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TestedRepr {
    Word(String),
    List(Vec<Direction>),
}

impl<'de> Deserialize<'de> for PlacementFilter {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let repr = FilterRepr::deserialize(d)?;
        let rule = match (repr.conditions, repr.predicate) {
            (Some(conditions), None) => PlacementRule::Conditions(short_form(conditions)?),
            (None, Some(predicate)) => PlacementRule::Predicate(predicate),
            _ => {
                return Err(de::Error::custom(
                    "a placement filter has exactly one of `conditions` and `predicate`",
                ));
            }
        };
        let tested_directions = repr.tested_directions.map(tested).transpose()?;
        Ok(PlacementFilter {
            rule,
            tested_directions,
        })
    }
}

fn short_form<E: de::Error>(conditions: Vec<ConditionRepr>) -> Result<Vec<PlacementCondition>, E> {
    if conditions.is_empty() {
        return Err(E::custom("`conditions` is empty"));
    }
    conditions
        .into_iter()
        .map(|condition| {
            let allowed_faces = condition
                .allowed_faces
                .unwrap_or_else(|| Direction::all().to_vec());
            if allowed_faces.is_empty() {
                return Err(E::custom("`allowed_faces` is empty"));
            }
            if condition.block_filter.is_empty() {
                return Err(E::custom("`block_filter` is empty"));
            }
            let block_filter = condition
                .block_filter
                .iter()
                .map(|name| entry(name))
                .collect::<Result<_, E>>()?;
            Ok(PlacementCondition {
                allowed_faces,
                block_filter,
            })
        })
        .collect()
}

fn tested<E: de::Error>(repr: TestedRepr) -> Result<DirectionSet, E> {
    match repr {
        TestedRepr::Word(word) if word == "all" => Ok(DirectionSet::all()),
        TestedRepr::Word(word) => Err(E::custom(format!(
            "`tested_directions` is `all` or a list of directions, not `{word}`"
        ))),
        TestedRepr::List(directions) if directions.is_empty() => {
            Err(E::custom("`tested_directions` is empty"))
        }
        TestedRepr::List(directions) => {
            let mut set = DirectionSet::default();
            for direction in directions {
                if set.contains(direction) {
                    return Err(E::custom(format!(
                        "`tested_directions` names `{direction}` two times"
                    )));
                }
                set.insert(direction);
            }
            Ok(set)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Result<PlacementFilter, String> {
        serde_json::from_str(text).map_err(|error| error.to_string())
    }

    #[test]
    fn the_load_refuses_a_filter_outside_the_form() {
        let refused = [
            r#"{}"#,
            r#"{"conditions": [], "predicate": {"type": "minecraft:true"}}"#,
            r#"{"conditions": []}"#,
            r#"{"conditions": [{"allowed_faces": [], "block_filter": ["minecraft:dirt"]}]}"#,
            r#"{"conditions": [{"block_filter": []}]}"#,
            r#"{"conditions": [{"block_filter": [{"tags": "q.any_tag('dirt')"}]}]}"#,
            r#"{"predicate": {"type": "minecraft:would_survive", "state": {"Name": "minecraft:stone"}}}"#,
            r#"{"predicate": {"type": "minecraft:height_range", "min_inclusive": {"absolute": 0}, "max_inclusive": {"absolute": 1}}}"#,
            r#"{"predicate": {"type": "minecraft:solid", "offset": [0, -2, 0]}}"#,
            r#"{"predicate": {"type": "minecraft:solid", "offset": [1, -1, 0]}}"#,
            r#"{"predicate": {"type": "minecraft:true"}, "tested_directions": []}"#,
            r#"{"predicate": {"type": "minecraft:true"}, "tested_directions": ["up", "up"]}"#,
            r#"{"predicate": {"type": "minecraft:true"}, "tested_directions": "some"}"#,
        ];
        for text in refused {
            assert!(read(text).is_err(), "{text}");
        }
    }

    #[test]
    fn the_load_reads_both_forms_with_their_defaults() {
        let short = read(r##"{"conditions": [{"block_filter": ["#minecraft:dirt"]}]}"##).unwrap();
        let PlacementRule::Conditions(conditions) = short.rule else {
            panic!("{short:?}");
        };
        assert_eq!(conditions[0].allowed_faces, Direction::all().to_vec());
        assert!(conditions[0].block_filter[0].tag);
        assert_eq!(short.tested_directions, None);

        let face = read(
            r#"{"predicate": {"type": "minecraft:has_sturdy_face", "offset": [0, -1, 0], "direction": "up"},
                "tested_directions": "all"}"#,
        )
        .unwrap();
        assert_eq!(
            face.rule,
            PlacementRule::Predicate(SupportPredicate::HasSturdyFace {
                offset: Offset(Some(Direction::Down)),
                direction: Direction::Up,
                face_test: FaceTest::Full,
                shape: FaceShape::Support,
            })
        );
        assert_eq!(face.tested_directions, Some(DirectionSet::all()));
    }

    #[test]
    fn a_support_with_a_bedrock_shape_name_is_refused() {
        assert!(serde_json::from_str::<Support>(r#"{"shape": "fence"}"#).is_err());
        assert!(serde_json::from_str::<Support>(r#"{"boxes": []}"#).is_ok());
    }
}
