use mcrs_vanilla::enchantment::data::{EnchantmentCost, NetworkEnchantmentData};
use serde::Deserialize;

#[derive(Deserialize)]
struct ProtoEnch {
    description: serde_json::Value,
    min_cost: EnchantmentCost,
    max_cost: EnchantmentCost,
    anvil_cost: u32,
    slots: Vec<String>,
    supported_items: String,
    #[serde(default)]
    primary_items: Option<String>,
    weight: u32,
    max_level: u32,
    #[serde(default)]
    exclusive_set: Option<String>,
    #[serde(default)]
    effects: Option<serde_json::Value>,
}

fn main() {
    let bytes = std::fs::read("assets/minecraft/enchantment/fortune.json").unwrap();
    let proto: ProtoEnch = serde_json::from_slice(&bytes).unwrap();
    let net = NetworkEnchantmentData {
        description: proto.description,
        min_cost: proto.min_cost,
        max_cost: proto.max_cost,
        anvil_cost: proto.anvil_cost,
        slots: proto.slots,
        supported_items: proto.supported_items,
        primary_items: proto.primary_items,
        weight: proto.weight,
        max_level: proto.max_level,
        exclusive_set: proto.exclusive_set,
        effects: proto.effects,
    };
    let nbt = mcrs_nbt::to_nbt_compound(&net).unwrap();
    println!("{:#?}", nbt);
}
