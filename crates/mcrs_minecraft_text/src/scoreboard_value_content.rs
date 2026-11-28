use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ScoreboardValueContent {
    pub name: Cow<'static, str>,
    pub objective: Cow<'static, str>,
}
