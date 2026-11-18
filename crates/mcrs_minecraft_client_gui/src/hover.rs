use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_menu_geometry::CreativeHit;
use mcrs_minecraft_menu_geometry::CreativeTabIndex;

use crate::creative_clicks;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Hover {
    RecipeBookButton,
    Menu(Clicked),
    Creative(CreativeHit),
}

impl Hover {
    pub(crate) fn clicked(self) -> Clicked {
        match self {
            Self::RecipeBookButton => Clicked::Nowhere,
            Self::Menu(clicked) => clicked,
            Self::Creative(hit) => creative_clicks::clicked(hit),
        }
    }

    pub(crate) fn menu_entry(self) -> Option<u16> {
        match self {
            Self::Creative(hit) => hit.menu_index(),
            Self::RecipeBookButton | Self::Menu(_) => match self.clicked() {
                Clicked::Entry(index) => Some(index),
                Clicked::Outside | Clicked::Nowhere => None,
            },
        }
    }

    pub(crate) fn takes_left_press(self) -> bool {
        matches!(
            self,
            Self::RecipeBookButton | Self::Creative(CreativeHit::Tab(_) | CreativeHit::ScrollBar)
        )
    }

    pub(crate) fn permits_drag(self) -> bool {
        match self {
            Self::Creative(hit) => creative_clicks::permits_drag(hit),
            Self::RecipeBookButton | Self::Menu(_) => true,
        }
    }

    pub(crate) fn tab(self) -> Option<CreativeTabIndex> {
        match self {
            Self::Creative(CreativeHit::Tab(tab)) => Some(tab),
            Self::RecipeBookButton | Self::Menu(_) | Self::Creative(_) => None,
        }
    }
}
