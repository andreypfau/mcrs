pub mod data;
pub mod effects;
pub mod predicate;
pub mod value;
pub mod registry;
pub mod tags;

pub(crate) use data::ProtoEnchantmentData;
pub use data::{EnchantmentCost, EnchantmentData, NetworkEnchantmentData};
pub use registry::{VANILLA_ENCHANTMENTS, register_all_enchantments};
