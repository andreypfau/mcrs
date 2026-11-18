use mcrs_minecraft_container_click::Clicked;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Hover {
    RecipeBookButton,
    Menu(Clicked),
}

impl Hover {
    pub(crate) fn clicked(self) -> Clicked {
        match self {
            Self::RecipeBookButton => Clicked::Nowhere,
            Self::Menu(clicked) => clicked,
        }
    }
}
