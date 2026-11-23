use crate::{
    AnimateObserved, CommandTree, ExpectedPacket, MenuEvent, PlayerBan, SoundPlayed, SwingObserved,
};
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
    pub swings: Option<Vec<SwingObserved>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swing_bytes: Option<SwingWireCheck>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animates: Option<Vec<AnimateObserved>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packets: Option<Vec<PacketObserved>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoints: Vec<Checkpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_file: Option<BTreeMap<String, Option<String>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Checkpoint {
    Packet {
        client: String,
        packet: ExpectedPacket,
    },
    GameTicks {
        ticks: u32,
    },
    PlayerTicks {
        client: String,
        at_least: u32,
    },
}

impl StepTrace {
    pub fn normalize_per_client(&mut self) {
        if let Some(swings) = &mut self.swings {
            swings.sort_by(|left, right| left.client.cmp(&right.client));
        }
        if let Some(animates) = &mut self.animates {
            animates.sort_by(|left, right| left.client.cmp(&right.client));
        }
        if let Some(packets) = &mut self.packets {
            packets.sort_by(|left, right| left.client.cmp(&right.client));
        }
        match &mut self.observed {
            Observed::Command { chat, .. } | Observed::Console { chat, .. } => {
                chat.sort_by(|left, right| left.client.cmp(&right.client));
            }
            Observed::Joined
            | Observed::Refused { .. }
            | Observed::Left
            | Observed::Suggestions { .. }
            | Observed::Sent { .. }
            | Observed::Restarted => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwingWireCheck {
    pub canonical: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_across_recipients: Option<bool>,
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
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        chat: Vec<ChatMessage>,
        disconnects: Vec<Disconnect>,
    },
    Suggestions {
        start: i32,
        length: i32,
        suggestions: Vec<String>,
        disconnects: Vec<Disconnect>,
    },
    Sent {
        disconnects: Vec<Disconnect>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        console_output: Vec<String>,
    },
    Console {
        output: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        chat: Vec<ChatMessage>,
    },
    Restarted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketObserved {
    pub client: String,
    pub packet: PacketKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PacketKind {
    SwingAnimation,
    Sound,
    BlockUpdate,
    SectionBlocksUpdate,
    SetEquipment,
    SetEntityData,
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
