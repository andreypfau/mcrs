use mcrs_minecraft_core::codec::is_default;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageType {
    pub message_id: String,
    pub scaling: DamageScaling,
    pub exhaustion: f32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub effects: DamageEffects,
    #[serde(default, skip_serializing_if = "is_default")]
    pub death_message_type: DeathMessageType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageScaling {
    Never,
    WhenCausedByLivingNonPlayer,
    Always,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageEffects {
    #[default]
    Hurt,
    Thorns,
    Drowning,
    Burning,
    Poking,
    Freezing,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathMessageType {
    #[default]
    Default,
    FallVariants,
    IntentionalGameDesign,
}
