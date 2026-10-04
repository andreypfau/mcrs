use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::{ResourceLocation, rl};
use mcrs_minecraft_item::Template;
use serde::{Deserialize, Serialize};

pub type Style = mcrs_minecraft_text::Style<Template>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatType {
    pub chat: ChatDecoration,
    pub narration: ChatDecoration,
}

impl RegistryKey for ChatType {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:chat_type");
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatDecoration {
    pub translation_key: String,
    pub parameters: Vec<ChatParameter>,
    #[serde(default, skip_serializing_if = "Style::is_empty")]
    pub style: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatParameter {
    Sender,
    Target,
    Content,
}
