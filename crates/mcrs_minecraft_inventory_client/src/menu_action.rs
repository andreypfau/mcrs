use mcrs_minecraft_container_click::Click;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MenuAction {
    Click(Click),
    Close,
}
