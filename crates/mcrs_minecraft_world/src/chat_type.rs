use mcrs_minecraft_item::Template;
use mcrs_minecraft_registries::{Refs, Resolved, registry_value};
use serde::{Deserialize, Serialize};

pub type Style<M = Resolved> = mcrs_minecraft_text::Style<Template<M>>;

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatType<M: Refs = Resolved> {
    pub chat: ChatDecoration<M>,
    pub narration: ChatDecoration<M>,
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatDecoration<M: Refs = Resolved> {
    pub translation_key: String,
    pub parameters: Vec<ChatParameter>,
    #[serde(default, skip_serializing_if = "Style::is_empty")]
    pub style: Style<M>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatParameter {
    Sender,
    Target,
    Content,
}
