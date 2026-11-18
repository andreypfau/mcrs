use mcrs_minecraft_container_click::Click;

use crate::CreativeArea;

#[derive(Clone, Debug, PartialEq)]
pub struct CreativeClick {
    pub area: CreativeArea,
    pub click: Click,
}
