use mcrs_minecraft_inventory_player::PlayerMode;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClickPlayer {
    pub mode: PlayerMode,
    pub selected: u8,
}
