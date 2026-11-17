use crate::{Click, PlayerActionStatus, PlayerBan, PredictedStack};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ScenarioFields")]
pub struct Scenario {
    #[serde(default)]
    pub target: Target,
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    #[serde(default)]
    pub operators: Vec<String>,
    #[serde(default)]
    pub banned_players: Vec<PlayerBan>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observe {
    #[serde(default, skip_serializing_if = "is_false")]
    pub inventory: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub sound: bool,
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
    pub id: String,
    #[serde(flatten)]
    pub action: Action,
    #[serde(default, skip_serializing_if = "is_false")]
    pub read_player_bans: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_player_file: Option<PlayerFileRead>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerFileRead {
    pub client: String,
    pub fields: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Join(String),
    Leave(String),
    Command {
        client: String,
        line: String,
    },
    Console {
        lines: Vec<String>,
    },
    Click(Click),
    Close {
        client: String,
        container: Option<i32>,
    },
    SelectSlot {
        client: String,
        slot: i16,
    },
    PlayerAction {
        client: String,
        status: PlayerActionStatus,
    },
    CreativeSlot {
        client: String,
        slot: i16,
        stack: Option<PredictedStack>,
    },
    UseBlock {
        client: String,
        position: [i32; 3],
    },
    RestartServer {},
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde passes the field by reference"
)]
fn is_false(value: &bool) -> bool {
    !*value
}
