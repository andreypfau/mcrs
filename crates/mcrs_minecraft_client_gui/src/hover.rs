use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_container_gesture::GestureHit;
use mcrs_minecraft_menu_geometry::{CreativeHit, CreativeTabIndex};

use crate::creative_clicks;
use crate::creative_grid::CreativeGrid;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Hover {
    RecipeBookButton,
    Menu(Clicked),
    Creative(CreativeHit),
}

impl Hover {
    pub(crate) fn hit(self, grid: Option<CreativeGrid>) -> GestureHit {
        match self {
            Self::RecipeBookButton => GestureHit {
                clicked: Some(Clicked::Nowhere),
                entry: None,
                target: None,
                takes_left_press: true,
                permits_drag: true,
            },
            Self::Menu(clicked) => GestureHit {
                clicked: Some(clicked),
                entry: match clicked {
                    Clicked::Entry(index) => Some(index),
                    Clicked::Outside | Clicked::Nowhere => None,
                },
                target: None,
                takes_left_press: false,
                permits_drag: true,
            },
            Self::Creative(hit) => creative_clicks::hit(hit, grid),
        }
    }

    pub(crate) fn tab(self) -> Option<CreativeTabIndex> {
        match self {
            Self::Creative(CreativeHit::Tab(tab)) => Some(tab),
            Self::RecipeBookButton | Self::Menu(_) | Self::Creative(_) => None,
        }
    }
}
