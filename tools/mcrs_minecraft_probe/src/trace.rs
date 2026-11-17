use crate::{CommandTree, MenuEvent, PlayerBan, SoundPlayed};
use mcrs_minecraft_text::Text;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    pub version: TraceVersion,
    pub steps: Vec<StepTrace>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceVersion {
    pub name: String,
    pub protocol: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepTrace {
    pub id: String,
    pub observed: Observed,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_bans: Option<Vec<PlayerBan>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inventory: Option<Vec<MenuEvent>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sounds: Option<Vec<SoundPlayed>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_trees: Option<Vec<CommandTree>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_file: Option<BTreeMap<String, Option<String>>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Observed {
    Joined,
    Refused {
        state: ConnectionState,
        reason: Option<Text>,
    },
    Left,
    Command {
        replies: Vec<Text>,
        disconnects: Vec<Disconnect>,
    },
    Sent {
        disconnects: Vec<Disconnect>,
    },
    Console {
        output: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        chat: Vec<ChatMessage>,
    },
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
