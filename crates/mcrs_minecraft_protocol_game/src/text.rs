//! Text components with the item stack template as the hover item, in the named form a game
//! packet carries.

pub use mcrs_minecraft_text::*;

pub type Text = mcrs_minecraft_item::Text<mcrs_minecraft_registries::Named>;
