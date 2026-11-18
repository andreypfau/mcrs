use bevy::prelude::Resource;
use mcrs_minecraft_client_widget::EditBox;
use mcrs_minecraft_client_widget::LengthUnit;
use mcrs_minecraft_creative_content::SearchQuery;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_menu_geometry::CREATIVE_TABS;
use mcrs_minecraft_menu_geometry::CreativeGeometry;
use mcrs_minecraft_menu_geometry::CreativeTab;
use mcrs_minecraft_menu_geometry::CreativeTabKind;

use crate::creative_tab_content::CreativeTabContent;
use crate::creative_tab_view::CreativeTabView;

const FIRST_TAB: usize = 0;
const SEARCH_MAX_LENGTH: usize = 50;
const SEARCH_TEXT_COLOR: u32 = 0xFFFF_FFFF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SearchReset {
    Clear,
    Keep,
}

#[derive(Resource, Clone, Debug)]
pub struct CreativeScreenState {
    selected: usize,
    scroll: f64,
    content_len: usize,
    search: EditBox,
    searched: String,
    result: Vec<usize>,
}

impl Default for CreativeScreenState {
    fn default() -> Self {
        let mut search = EditBox::default();
        search.set_max_length(SEARCH_MAX_LENGTH, LengthUnit::CodeUnits);
        search.border = false;
        search.text_color = SEARCH_TEXT_COLOR;
        search.invert = false;
        search.can_lose_focus = false;
        Self {
            selected: FIRST_TAB,
            scroll: 0.0,
            content_len: 0,
            search,
            searched: String::new(),
            result: Vec::new(),
        }
    }
}

impl CreativeScreenState {
    pub fn open(&mut self, content: &CreativeTabContent) {
        let tab = if content.visible_tabs().contains(self.selected) {
            self.selected
        } else {
            FIRST_TAB
        };
        self.apply_selection(tab, SearchReset::Clear, content);
    }

    pub fn select(&mut self, tab: usize, content: &CreativeTabContent) {
        let reset = if tab == self.selected && self.selected_tab().kind == CreativeTabKind::Search {
            SearchReset::Keep
        } else {
            SearchReset::Clear
        };
        self.apply_selection(tab, reset, content);
    }

    pub fn wheel(&mut self, vertical: f64, content: &CreativeTabContent) {
        let len = self.content(content).len();
        if !CreativeGeometry::can_scroll(self.selected_tab().kind, len) {
            return;
        }
        let rows = f64::from(CreativeGeometry::row_count(len));
        self.scroll = (self.scroll - vertical / rows).clamp(0.0, 1.0);
    }

    pub fn drag_scroll(&mut self, top: i32, mouse_y: f64, content: &CreativeTabContent) {
        if self.can_scroll(content) {
            self.scroll = CreativeGeometry::scroll_from_drag(top, mouse_y);
        }
    }

    pub fn edit_search(&mut self, content: &CreativeTabContent, edit: impl FnOnce(&mut EditBox)) {
        edit(&mut self.search);
        if self.search.text() != self.searched {
            self.match_search(content);
            self.scroll = 0.0;
            self.content_len = self.content(content).len();
        }
    }

    pub fn content_changed(&mut self, content: &CreativeTabContent) {
        self.match_search(content);
        if self.selected_tab().kind == CreativeTabKind::Items
            && content.tab_list(self.selected).is_empty()
        {
            self.apply_selection(FIRST_TAB, SearchReset::Clear, content);
            return;
        }
        let before = CreativeGeometry::row_count(self.content_len);
        let first_row = CreativeGeometry::first_row(self.scroll, before);
        let len = self.content(content).len();
        let rows = CreativeGeometry::row_count(len);
        self.scroll = if rows >= 1 {
            (f64::from(first_row) / f64::from(rows)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.content_len = len;
    }

    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    #[must_use]
    pub fn selected_tab(&self) -> CreativeTab {
        let [first, ..] = CREATIVE_TABS;
        CREATIVE_TABS.get(self.selected).copied().unwrap_or(first)
    }

    #[must_use]
    pub fn scroll(&self) -> f64 {
        self.scroll
    }

    #[must_use]
    pub fn search_box(&self) -> &EditBox {
        &self.search
    }

    #[must_use]
    pub fn content<'a>(&'a self, content: &'a CreativeTabContent) -> CreativeTabView<'a> {
        match self.selected_tab().kind {
            CreativeTabKind::Items => CreativeTabView::Stacks(content.tab_list(self.selected)),
            CreativeTabKind::Search => CreativeTabView::Search {
                list: content.search_list(),
                result: &self.result,
            },
            CreativeTabKind::SavedHotbars | CreativeTabKind::SurvivalInventory => {
                CreativeTabView::Stacks(&[])
            }
        }
    }

    #[must_use]
    pub fn can_scroll(&self, content: &CreativeTabContent) -> bool {
        CreativeGeometry::can_scroll(self.selected_tab().kind, self.content(content).len())
    }

    #[must_use]
    pub fn first_row(&self, content: &CreativeTabContent) -> i32 {
        let rows = CreativeGeometry::row_count(self.content(content).len());
        CreativeGeometry::first_row(self.scroll, rows)
    }

    #[must_use]
    pub fn grid_stack<'a>(
        &'a self,
        slot: usize,
        content: &'a CreativeTabContent,
    ) -> Option<&'a ItemStack> {
        let index =
            CreativeGeometry::content_index(u16::try_from(slot).ok()?, self.first_row(content))?;
        self.content(content).get(index)
    }

    fn apply_selection(&mut self, tab: usize, reset: SearchReset, content: &CreativeTabContent) {
        if tab >= CREATIVE_TABS.len() {
            return;
        }
        self.selected = tab;
        if reset == SearchReset::Clear {
            self.search.set("");
        }
        if self.search.text() != self.searched {
            self.match_search(content);
        }
        self.scroll = 0.0;
        self.content_len = self.content(content).len();
    }

    fn match_search(&mut self, content: &CreativeTabContent) {
        self.searched.clear();
        self.searched.push_str(self.search.text());
        let query = SearchQuery::new(&self.searched);
        let index = content.search_index();
        self.result.clear();
        self.result
            .extend((0..content.search_list().len()).filter(|stack| index.matches(*stack, &query)));
    }
}
