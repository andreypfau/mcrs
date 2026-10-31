use mcrs_minecraft_item::keys::Item;
use serde::{Deserialize, Serialize};

use crate::SavedItem;

const VAULT_ACTIVATION_RANGE: f64 = 4.0;
const VAULT_DEACTIVATION_RANGE: f64 = 4.5;

fn is_vault_loot(value: &str) -> bool {
    mcrs_minecraft_item::keys::loot_table::CHESTS_TRIAL_CHAMBERS_REWARD.location() == value
}

fn is_vault_activation_range(value: &f64) -> bool {
    *value == VAULT_ACTIVATION_RANGE
}

fn is_vault_deactivation_range(value: &f64) -> bool {
    *value == VAULT_DEACTIVATION_RANGE
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VaultConfig {
    #[serde(skip_serializing_if = "is_vault_loot")]
    pub loot_table: String,
    #[serde(skip_serializing_if = "is_vault_activation_range")]
    pub activation_range: f64,
    #[serde(skip_serializing_if = "is_vault_deactivation_range")]
    pub deactivation_range: f64,
    pub key_item: SavedItem,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_loot_table_to_display: Option<String>,
}

impl Default for VaultConfig {
    fn default() -> Self {
        VaultConfig {
            loot_table: mcrs_minecraft_item::keys::loot_table::CHESTS_TRIAL_CHAMBERS_REWARD
                .to_string(),
            activation_range: VAULT_ACTIVATION_RANGE,
            deactivation_range: VAULT_DEACTIVATION_RANGE,
            key_item: SavedItem {
                id: Item::TrialKey.as_static_str().to_owned(),
                count: 1,
                components: None,
            },
            override_loot_table_to_display: None,
        }
    }
}
