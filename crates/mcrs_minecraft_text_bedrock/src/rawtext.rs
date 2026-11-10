use serde::{Deserialize, Serialize};

/// Root Bedrock `rawtext` structure.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BedrockRawtext {
    pub rawtext: Vec<BedrockRawtextComponent>,
}

/// A component inside a Bedrock `rawtext` array.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum BedrockRawtextComponent {
    /// Literal text with optional `§` formatting codes.
    Text { text: String },
    /// Localized translation component resolved on the Bedrock client.
    Translate {
        translate: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        with: Option<BedrockRawtextWith>,
    },
    /// A group of components that counts as one argument of a translation.
    Rawtext {
        rawtext: Vec<BedrockRawtextComponent>,
    },
}

/// Arguments to a Bedrock translation component.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum BedrockRawtextWith {
    /// Plain string arguments.
    Strings(Vec<String>),
    /// Nested rawtext components (used when arguments contain styles or translations).
    Rawtext {
        rawtext: Vec<BedrockRawtextComponent>,
    },
}
