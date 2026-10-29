use mcrs_minecraft_nbt::nbt_flag;
use serde::{Deserialize, Serialize};

use crate::Text;

fn black() -> String {
    "black".to_owned()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignText {
    pub messages: Vec<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filtered_messages: Option<Vec<Text>>,
    #[serde(default = "black")]
    pub color: String,
    #[serde(default, deserialize_with = "nbt_flag")]
    pub has_glowing_text: bool,
}

impl Default for SignText {
    fn default() -> Self {
        Self {
            messages: vec![Text::Plain(String::new()); 4],
            filtered_messages: None,
            color: black(),
            has_glowing_text: false,
        }
    }
}
