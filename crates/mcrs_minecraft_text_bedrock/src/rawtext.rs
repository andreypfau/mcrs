use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BedrockRawtext {
    pub rawtext: Vec<BedrockRawtextComponent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum BedrockRawtextComponent {
    Text {
        text: String,
    },
    Translate {
        translate: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        with: Option<BedrockRawtextWith>,
    },
    Rawtext {
        rawtext: Vec<BedrockRawtextComponent>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum BedrockRawtextWith {
    Strings(Vec<String>),
    Rawtext {
        rawtext: Vec<BedrockRawtextComponent>,
    },
}
