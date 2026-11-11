use crate::PlayerBan;
use mcrs_minecraft_text::Text;
use serde::{Deserialize, Serialize};

/// What the clients of one scenario observed, in the order of its steps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    pub version: TraceVersion,
    pub steps: Vec<StepTrace>,
}

/// The game version the clients spoke.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceVersion {
    pub name: String,
    pub protocol: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepTrace {
    pub id: String,
    pub observed: Observed,
    /// The player ban file after the step, sorted by name, when the scenario asks for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_bans: Option<Vec<PlayerBan>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Observed {
    /// The client reached the game state and stayed in it.
    Joined,
    /// The server ended the connection of the client that joins.
    Refused {
        state: ConnectionState,
        /// Missing when the server closed the connection without a disconnect packet.
        reason: Option<Text>,
    },
    Left,
    Command {
        /// The system chat messages the sender received, in the order of their arrival.
        replies: Vec<Text>,
        /// The clients the server disconnected, sorted by name.
        disconnects: Vec<Disconnect>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Login,
    Configuration,
    Play,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Disconnect {
    pub client: String,
    pub reason: Text,
}
