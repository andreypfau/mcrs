#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum IntValueProvider {
    Constant(i32),
    Tagged(TaggedIntValueProvider),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type")]
pub enum TaggedIntValueProvider {
    #[serde(rename = "minecraft:constant")]
    Constant { value: i32 },

    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: i32,
        max_inclusive: i32,
    },
}
