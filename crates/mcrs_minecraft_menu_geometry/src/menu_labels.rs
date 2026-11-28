use mcrs_minecraft_client_widget::GuiPoint;

use crate::TitlePosition;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct MenuLabels {
    pub title: TitlePosition,
    pub player_inventory: GuiPoint,
}
