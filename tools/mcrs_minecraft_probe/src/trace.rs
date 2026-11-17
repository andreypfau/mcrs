use crate::{CommandTree, MenuEvent, PlayerBan, SoundPlayed};
use mcrs_minecraft_text::Text;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    /// The menu packets that the clients received during the step, in the order of their
    /// arrival, when the scenario observes the inventory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inventory: Option<Vec<MenuEvent>>,
    /// The sound packets that the clients received during the step, in the order of their
    /// arrival, when the scenario observes sound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sounds: Option<Vec<SoundPlayed>>,
    /// The command trees that the clients received during the step, in the order of their
    /// arrival, when the scenario names the commands to observe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_trees: Option<Vec<CommandTree>>,
    /// The named fields of the saved player file after the step, as SNBT; `null` for a field
    /// that the file does not have.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_file: Option<BTreeMap<String, Option<String>>>,
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
    /// The client sent a game packet.
    Sent {
        /// The clients the server disconnected, sorted by name.
        disconnects: Vec<Disconnect>,
    },
    /// The host ran commands on the console of the server.
    Console {
        /// The replies that the server printed, in order.
        output: Vec<String>,
        /// The system chat messages the clients received, in the order of their arrival.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        chat: Vec<ChatMessage>,
    },
    /// The host stopped the server and started it again.
    Restarted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Login,
    Configuration,
    Play,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub client: String,
    pub text: Text,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Disconnect {
    pub client: String,
    pub reason: Text,
}
