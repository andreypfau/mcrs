use mcrs_minecraft_container_click::Click;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureClick {
    Hit(Click),
    Menu(Click),
}
