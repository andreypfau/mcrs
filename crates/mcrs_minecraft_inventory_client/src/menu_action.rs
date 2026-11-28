use mcrs_minecraft_container_click::Click;

use crate::CreativeClick;

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum MenuAction {
    Click(Click),
    Creative(CreativeClick),
    Close,
}
