pub mod builder;
pub mod comparator;
pub use builder::from_input;
pub use comparator::{assert_nibbles_eq, get_nibble, nibble_index, set_nibble};
