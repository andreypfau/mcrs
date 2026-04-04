pub mod data;
pub mod registry;
pub mod tags;

pub use data::{EnchantmentCost, EnchantmentData};
pub use registry::{register_all_enchantments, VANILLA_ENCHANTMENTS};
