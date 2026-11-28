use mcrs_minecraft_item_stack::MapId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapSummary {
    pub id: MapId,
    pub locked: bool,
    pub scale: u8,
}
