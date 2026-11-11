use crate::PlayerBan;
use serde::{Deserialize, Serialize};

/// One experiment: the state of the target before its start, and the steps of the clients.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scenario {
    #[serde(default)]
    pub target: Target,
    pub steps: Vec<Step>,
}

/// What the host of a target applies before the first step. The driver does not read it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    /// The names of the players that are operators.
    #[serde(default)]
    pub operators: Vec<String>,
    /// The content of the player ban file when the server starts.
    #[serde(default)]
    pub banned_players: Vec<PlayerBan>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// Names the step in the trace; unique in a scenario.
    pub id: String,
    #[serde(flatten)]
    pub action: Action,
    /// The trace of this step holds the player ban file as it is after the step.
    #[serde(default, skip_serializing_if = "is_false")]
    pub read_player_bans: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// A new client with this player name logs in.
    Join(String),
    /// The client with this player name closes its connection.
    Leave(String),
    /// The client sends a chat command, written without the slash.
    Command { client: String, line: String },
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde passes the field by reference"
)]
fn is_false(value: &bool) -> bool {
    !*value
}
