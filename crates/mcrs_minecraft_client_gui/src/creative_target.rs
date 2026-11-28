use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_inventory_client::CreativeArea;

pub(crate) struct CreativeTarget {
    pub(crate) clicked: Clicked,
    pub(crate) area: CreativeArea,
}
