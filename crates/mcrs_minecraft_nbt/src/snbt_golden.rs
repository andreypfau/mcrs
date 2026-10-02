// Values printed by the game's own functions: TagParser + Tag.toString +
// NbtIo.writeAnyTag, JsonOps.convertTo(NbtOps) and Double/Float.toString.

use std::sync::LazyLock;

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Parse {
    pub input: String,
    pub hex: String,
    pub snbt: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DoubleText {
    pub bits: u64,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FloatText {
    pub bits: u32,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JsonTyping {
    pub json: String,
    pub tag: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Golden {
    pub parses: Vec<Parse>,
    pub rejects: Vec<String>,
    pub doubles: Vec<DoubleText>,
    pub floats: Vec<FloatText>,
    pub json_typing: Vec<JsonTyping>,
    pub pretty: String,
    pub pretty_hex: String,
}

pub(crate) static GOLDEN: LazyLock<Golden> =
    LazyLock::new(|| serde_json::from_str(include_str!("fixtures/snbt.json")).unwrap());
