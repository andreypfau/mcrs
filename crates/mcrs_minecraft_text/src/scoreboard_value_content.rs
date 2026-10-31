use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// Scoreboard value.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ScoreboardValueContent {
    /// The name of the score holder whose score should be displayed. This
    /// can be a [`selector`] or an explicit name.
    ///
    /// [`selector`]: https://minecraft.fandom.com/wiki/Target_selectors
    pub name: Cow<'static, str>,
    /// The internal name of the objective to display the player's score in.
    pub objective: Cow<'static, str>,
}
