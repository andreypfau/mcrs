use crate::run::is_sequence_step;
use crate::{Click, Direction, PlayerActionStatus, PlayerBan, PredictedStack, ProbeHand};
use mcrs_minecraft_protocol::setting::MainArm;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    type Error = ScenarioError;

    fn try_from(fields: ScenarioFields) -> Result<Self, Self::Error> {
        validate_operator_levels(&fields.target)?;
        for step in &fields.steps {
            validate_action(
                &step.id,
                &step.action,
                &fields.observe,
                ActionScope::TopLevel,
            )?;
        }
        Ok(Scenario {
            target: fields.target,
            observe: fields.observe,
            steps: fields.steps,
        })
    }
}

fn validate_operator_levels(target: &Target) -> Result<(), ScenarioError> {
    for (name, &level) in &target.operator_levels {
        if !(1..=4).contains(&level) {
            return Err(ScenarioError::InvalidOperatorLevel {
                name: name.clone(),
                level,
            });
        }
        if !target.operators.iter().any(|operator| operator == name) {
            return Err(ScenarioError::UnknownOperatorLevel { name: name.clone() });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActionScope {
    TopLevel,
    SequenceChild,
}

fn validate_action(
    step_id: &str,
    action: &Action,
    observe: &Observe,
    scope: ActionScope,
) -> Result<(), ScenarioError> {
    if !observe.inventory && matches!(action, Action::Click(_) | Action::Close { .. }) {
        return Err(ScenarioError::MenuViewNeeded {
            step: step_id.to_owned(),
        });
    }
    if !crate::client::needs_world_acks(observe) && matches!(action, Action::SyncPosition(_)) {
        return Err(ScenarioError::WorldAcksNeeded {
            step: step_id.to_owned(),
        });
    }
    if !observe.swing
        && matches!(
            action,
            Action::AwaitPacket {
                packet: ExpectedPacket::Swing { .. },
                ..
            }
        )
    {
        return Err(ScenarioError::SwingObservationNeeded {
            step: step_id.to_owned(),
        });
    }
    match action {
        Action::Sequence { steps } => {
            if scope == ActionScope::SequenceChild {
                return Err(ScenarioError::UnsupportedSequenceStep {
                    step: step_id.to_owned(),
                });
            }
            for child in steps {
                validate_action(step_id, child, observe, ActionScope::SequenceChild)?;
            }
            Ok(())
        }
        _ if scope == ActionScope::SequenceChild && !is_sequence_step(action) => {
            Err(ScenarioError::UnsupportedSequenceStep {
                step: step_id.to_owned(),
            })
        }
        _ => Ok(()),
    }
}

#[derive(Debug)]
pub(crate) enum ScenarioError {
    MenuViewNeeded { step: String },
    WorldAcksNeeded { step: String },
    SwingObservationNeeded { step: String },
    UnsupportedSequenceStep { step: String },
    InvalidOperatorLevel { name: String, level: u8 },
    UnknownOperatorLevel { name: String },
}

impl fmt::Display for ScenarioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MenuViewNeeded { step } => write!(
                formatter,
                "the step {step} sends a click or a close, which uses the menu packets; \
                 set `observe.inventory` to true"
            ),
            Self::WorldAcksNeeded { step } => write!(
                formatter,
                "the step {step} uses `sync_position`, which requires world acknowledgements; \
                 set at least one of `observe.inventory`, `observe.swing`, `observe.animate`, \
                 or `observe.packet_order` to true"
            ),
            Self::SwingObservationNeeded { step } => write!(
                formatter,
                "the step {step} waits for a swing packet; set `observe.swing` to true"
            ),
            Self::UnsupportedSequenceStep { step } => write!(
                formatter,
                "the step {step} has an unsupported action inside `sequence`"
            ),
            Self::InvalidOperatorLevel { name, level } => write!(
                formatter,
                "operator level for `{name}` must be between 1 and 4, got {level}"
            ),
            Self::UnknownOperatorLevel { name } => write!(
                formatter,
                "`operator_levels` references `{name}`, which is not in `target.operators`"
            ),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    #[serde(default)]
    pub operators: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub operator_levels: BTreeMap<String, u8>,
    #[serde(default)]
    pub banned_players: Vec<PlayerBan>,
}

impl Target {
    #[must_use]
    pub fn operator_level(&self, name: &str) -> u8 {
        self.operator_levels.get(name).copied().unwrap_or(4)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observe {
    #[serde(default, skip_serializing_if = "is_false")]
    pub inventory: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub sound: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub swing: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub animate: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub packet_order: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub chat: bool,
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
    #[serde(default, skip_serializing_if = "is_false")]
    pub packet_order: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeMainArm {
    Left,
    Right,
}

impl From<ProbeMainArm> for MainArm {
    fn from(arm: ProbeMainArm) -> Self {
        match arm {
            ProbeMainArm::Left => Self::Left,
            ProbeMainArm::Right => Self::Right,
        }
    }
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
    JoinDeferred(String),
    SendLoaded(String),
    SyncPosition(String),
    Leave(String),
    Command {
        client: String,
        line: String,
    },
    Suggest {
        client: String,
        line: String,
    },
    ClientMainArm {
        client: String,
        arm: ProbeMainArm,
    },
    PunchExtraByte {
        client: String,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        position: Option<[i32; 3]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        face: Option<Direction>,
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
    Punch {
        client: String,
    },
    PunchBurst {
        client: String,
        count: u32,
    },
    UseItem {
        client: String,
        hand: ProbeHand,
        sequence: i32,
    },
    UseItemOn {
        client: String,
        hand: ProbeHand,
        position: [i32; 3],
        face: Direction,
        sequence: i32,
    },
    Attack {
        client: String,
        target: String,
    },
    Interact {
        client: String,
        target: String,
        hand: ProbeHand,
        #[serde(default, skip_serializing_if = "is_false")]
        secondary: bool,
    },
    Wait {
        ms: u64,
    },
    AwaitPacket {
        client: String,
        packet: ExpectedPacket,
    },
    AdvanceTicks {
        ticks: u32,
    },
    AwaitPlayerTicks {
        client: String,
        ticks: u32,
    },
    Sequence {
        steps: Vec<Action>,
    },
    RestartServer {},
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedPacket {
    Swing { subject: String },
    BlockAck { sequence: i32 },
    AddEntity { subject: String },
    RemoveEntity { subject: String },
    PlayerPosition { block_center: [i32; 3] },
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde passes the field by reference"
)]
fn is_false(value: &bool) -> bool {
    !*value
}

pub(crate) fn needs_pose(steps: &[Step]) -> bool {
    steps.iter().any(|step| action_uses_pose(&step.action))
}

fn action_uses_pose(action: &Action) -> bool {
    match action {
        Action::Attack { .. }
        | Action::Interact { .. }
        | Action::UseItem { .. }
        | Action::AwaitPacket {
            packet: ExpectedPacket::PlayerPosition { .. },
            ..
        } => true,
        Action::Sequence { steps } => steps.iter().any(action_uses_pose),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_pose_detects_top_level_use_item_and_nested_target() {
        let passive: Scenario =
            serde_json::from_str(r#"{ "steps": [{ "id": "join", "join": "Alice" }] }"#).unwrap();
        assert!(
            !needs_pose(&passive.steps),
            "joining alone does not need pose"
        );

        let use_item: Scenario = serde_json::from_str(
            r#"{ "steps": [{ "id": "use", "use_item": { "client": "Alice", "hand": "main", "sequence": 1 } }] }"#,
        )
        .unwrap();
        assert!(
            needs_pose(&use_item.steps),
            "use_item sends the accepted look"
        );

        let nested: Scenario = serde_json::from_str(
            r#"{
                "steps": [{
                    "id": "seq",
                    "sequence": {
                        "steps": [{ "interact": { "client": "Alice", "target": "minecraft:pig", "hand": "main" } }]
                    }
                }]
            }"#,
        )
        .unwrap();
        assert!(
            needs_pose(&nested.steps),
            "interact needs the current position"
        );
    }
}
