use crate::{Click, PlayerActionStatus, PlayerBan, PredictedStack};
use serde::{Deserialize, Serialize};
use std::fmt;

/// One experiment: the state of the target before its start, and the steps of the clients.
///
/// A click or a close needs the menu packets, so a scenario with one of them and without
/// `observe.inventory` does not load.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ScenarioFields")]
pub struct Scenario {
    #[serde(default)]
    pub target: Target,
    /// What the trace records beside the result of each step.
    #[serde(default, skip_serializing_if = "Observe::is_default")]
    pub observe: Observe,
    pub steps: Vec<Step>,
}

#[derive(Deserialize)]
struct ScenarioFields {
    #[serde(default)]
    target: Target,
    #[serde(default)]
    observe: Observe,
    steps: Vec<Step>,
}

impl TryFrom<ScenarioFields> for Scenario {
    type Error = MenuViewNeeded;

    fn try_from(fields: ScenarioFields) -> Result<Self, Self::Error> {
        if !fields.observe.inventory {
            let needs_view = fields
                .steps
                .iter()
                .find(|step| matches!(step.action, Action::Click(_) | Action::Close { .. }));
            if let Some(step) = needs_view {
                return Err(MenuViewNeeded {
                    step: step.id.clone(),
                });
            }
        }
        Ok(Scenario {
            target: fields.target,
            observe: fields.observe,
            steps: fields.steps,
        })
    }
}

/// A step clicks or closes a container, but the clients do not read the menu packets.
#[derive(Debug)]
pub(crate) struct MenuViewNeeded {
    step: String,
}

impl fmt::Display for MenuViewNeeded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the step {} sends a click or a close, which uses the menu packets; \
             set `observe.inventory` to true",
            self.step
        )
    }
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observe {
    /// The clients answer position syncs and chunk batches, and the trace of each step holds the
    /// menu packets that they received.
    #[serde(default, skip_serializing_if = "is_false")]
    pub inventory: bool,
    /// The trace of each step holds the sound packets that the clients received.
    #[serde(default, skip_serializing_if = "is_false")]
    pub sound: bool,
    /// The trace of each step holds the subtrees of these literal children of the root of each
    /// `commands` packet that the clients received.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<String>,
}

impl Observe {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
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
    /// The trace of this step holds these fields of the saved player file after the step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_player_file: Option<PlayerFileRead>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerFileRead {
    pub client: String,
    /// Top-level field names of the player file.
    pub fields: Vec<String>,
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
    /// The host runs these commands on the server console, in order, and the trace holds the
    /// replies that the server printed.
    Console { lines: Vec<String> },
    /// The client sends a container click.
    Click(Click),
    /// The client closes a container, by default the one it saw open last.
    Close {
        client: String,
        container: Option<i32>,
    },
    /// The client selects a hotbar slot; the value is sent as is.
    SelectSlot { client: String, slot: i16 },
    /// The client sends a player action with this status at 0 0 0, face down, sequence 0.
    PlayerAction {
        client: String,
        status: PlayerActionStatus,
    },
    /// The client sets a slot of its inventory in creative mode.
    CreativeSlot {
        client: String,
        slot: i16,
        stack: Option<PredictedStack>,
    },
    /// The client uses its main hand on the top face of the block, at the centre of the face.
    UseBlock { client: String, position: [i32; 3] },
    /// The host stops the server, so that it saves, and starts it on the same world and port.
    RestartServer {},
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde passes the field by reference"
)]
fn is_false(value: &bool) -> bool {
    !*value
}
