use std::hash::{BuildHasher, Hasher, RandomState};
use std::time::Duration;

use bevy::prelude::Resource;
use mcrs_minecraft_client_widget::{
    Alignment, Child, CycleButton, EditBox, EditBoxHint, Grid, GridChild, GuiExtent, GuiInput,
    GuiRect, LayoutCell, LengthUnit, Overflow, Padding, Stack, Tab, TabBar, TooltipDelay, Widget,
    WidgetEvent, WidgetId, WidgetKind, WidgetValue, Widgets, text_line_extent,
};
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_save::{NewWorld, NewWorldMode};

use crate::font::{Font, Span};
use crate::item_decorations::WHITE;
use crate::language::ClientLanguage;
use crate::menu::{Menu, MenuBackground, MenuSeparators, WidgetHint, WidgetTooltip};
use crate::option_caption::option_caption;

pub(crate) const TABS: WidgetId = WidgetId(0);
pub(crate) const NAME_LABEL: WidgetId = WidgetId(1);
pub(crate) const NAME: WidgetId = WidgetId(2);
pub(crate) const GAME_MODE: WidgetId = WidgetId(3);
pub(crate) const WORLD_TYPE: WidgetId = WidgetId(4);
pub(crate) const SEED_LABEL: WidgetId = WidgetId(5);
pub(crate) const SEED: WidgetId = WidgetId(6);
pub(crate) const CREATE: WidgetId = WidgetId(7);
pub(crate) const CANCEL: WidgetId = WidgetId(8);

const TAB_WIDGETS: [[WidgetId; 3]; 2] = [
    [NAME_LABEL, NAME, GAME_MODE],
    [WORLD_TYPE, SEED_LABEL, SEED],
];

const GAME_MODES: [(NewWorldMode, &str); 3] = [
    (NewWorldMode::Survival, "survival"),
    (NewWorldMode::Hardcore, "hardcore"),
    (NewWorldMode::Creative, "creative"),
];
const DEFAULT_WORLD_TYPE: &str = "minecraft:normal";
const AMPLIFIED: &str = "minecraft:amplified";

const FOOTER_HEIGHT: i32 = 33;
const FOOTER_LINE: i32 = 35;
const FOOTER_ROW_TOP: i32 = 26;
const FOOTER_GAP: i32 = 8;
const ROW_GAP: i32 = 8;
const LABEL_GAP: i32 = 4;
const WORLD_COLUMN_GAP: i32 = 10;
const MAX_LENGTH: usize = 32;
const FOOTER_BUTTON: GuiExtent = GuiExtent::new(150, 20);
const NAME_BOX: GuiExtent = GuiExtent::new(208, 20);
const GAME_ROW: GuiExtent = GuiExtent::new(210, 20);
const HALF_ROW: GuiExtent = GuiExtent::new(150, 20);
const SEED_BOX: GuiExtent = GuiExtent::new(308, 20);
const SWITCHES: GuiExtent = GuiExtent::new(310, 48);

const ACROSS: LayoutCell = LayoutCell {
    padding: Padding {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    },
    alignment: Alignment { x: 0.5, y: 0.0 },
};

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct NewWorldForm {
    pub(crate) game_mode: NewWorldMode,
    pub(crate) world_type: Option<usize>,
    pub(crate) random_seed: i64,
}

impl NewWorldForm {
    pub(crate) fn opened(elapsed: Duration) -> Self {
        Self {
            random_seed: random_seed(elapsed),
            ..Self::default()
        }
    }

    pub(crate) fn seed_emptied(&mut self, elapsed: Duration) {
        self.random_seed = random_seed(elapsed);
    }

    fn world_type_index(&self, types: &[ResourceLocationBuf]) -> usize {
        self.world_type
            .or_else(|| {
                types
                    .iter()
                    .position(|preset| *preset == DEFAULT_WORLD_TYPE)
            })
            .unwrap_or(0)
    }

    pub(crate) fn write(&mut self, widget: &Widget) {
        let Some(WidgetValue::Choice(index)) = widget.value() else {
            return;
        };
        match widget.id() {
            GAME_MODE => {
                if let Some((mode, _)) = GAME_MODES.get(index) {
                    self.game_mode = *mode;
                }
            }
            WORLD_TYPE => self.world_type = Some(index),
            _ => {}
        }
    }

    pub(crate) fn new_world(
        &self,
        widgets: &Widgets,
        types: &[ResourceLocationBuf],
    ) -> Option<NewWorld> {
        Some(NewWorld {
            name: edit_text(widgets, NAME)?.to_owned(),
            game_mode: self.game_mode,
            world_type: types.get(self.world_type_index(types))?.clone(),
            seed_text: edit_text(widgets, SEED)?.to_owned(),
            random_seed: self.random_seed,
        })
    }
}

fn random_seed(elapsed: Duration) -> i64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u128(elapsed.as_nanos());
    i64::from_ne_bytes(hasher.finish().to_ne_bytes())
}

pub(crate) fn show_tab(widgets: &mut Widgets, tab: usize, events: &mut Vec<WidgetEvent>) {
    for (index, ids) in TAB_WIDGETS.iter().enumerate() {
        for id in ids {
            if index != tab {
                widgets.hide(*id, events);
            } else if let Some(widget) = widgets.get_mut(*id) {
                widget.visible = true;
            }
        }
    }
}

pub(crate) fn edit_text(widgets: &Widgets, id: WidgetId) -> Option<&str> {
    match widgets.value(id)? {
        WidgetValue::Text(text) => Some(text),
        _ => None,
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one builder lists the nine widgets of the screen with their captions and tooltips"
)]
pub(crate) fn create_world_menu(
    screen: GuiExtent,
    input: GuiInput,
    language: &ClientLanguage,
    font: &Font,
    types: &[ResourceLocationBuf],
    form: &NewWorldForm,
) -> Option<Menu> {
    let tab = |key| Tab {
        label: language.get(key).to_owned(),
        active: true,
    };
    let tabs = TabBar::new(vec![
        tab("createWorld.tab.game.title"),
        tab("createWorld.tab.world.title"),
    ]);
    let name_label = language.get("selectWorld.enterName");
    let seed_label = language.get("selectWorld.enterSeed");
    let label_size = |text| text_line_extent(font.str_width(text), None);
    let [
        name_label_at,
        name,
        game_mode,
        world_type,
        seed_label_at,
        seed,
        create,
        cancel,
    ] = layout(screen, label_size(name_label), label_size(seed_label))?;
    let mode_index = GAME_MODES
        .iter()
        .position(|(mode, _)| *mode == form.game_mode)
        .unwrap_or(0);
    let mode_name = GAME_MODES.get(mode_index).map_or("", |(_, name)| name);
    let mode_key = format!("selectWorld.gameMode.{mode_name}");
    let type_index = form.world_type_index(types);
    let preset = types.get(type_index);
    let type_key =
        preset.map(|preset| format!("generator.{}.{}", preset.namespace(), preset.path()));
    let type_value = type_key.as_deref().map_or("", |key| language.get(key));
    let text = |id, rect, caption: &str| {
        let kind = WidgetKind::Text {
            max_width: None,
            overflow: Overflow::Cut,
        };
        Widget::new(id, rect, caption.to_owned(), kind)
    };
    let edit_box = |id, rect, text: Option<&str>| {
        let mut edit = EditBox::default();
        edit.set_max_length(MAX_LENGTH, LengthUnit::CodeUnits);
        if let Some(text) = text {
            edit.set(text);
        }
        Widget::new(id, rect, String::new(), WidgetKind::EditBox(edit))
    };
    let cycle = |id, rect, name_key, value, cycle: Option<CycleButton>| {
        let caption = option_caption(language, "options.generic_value", name_key, value);
        let (kind, active) = cycle.map_or((WidgetKind::Button, false), |cycle| {
            (WidgetKind::CycleButton(cycle), true)
        });
        let mut widget = Widget::new(id, rect, caption, kind);
        widget.active = active;
        widget.tooltip = Some(TooltipDelay::default());
        widget
    };
    let button =
        |id, rect, key| Widget::new(id, rect, language.get(key).to_owned(), WidgetKind::Button);
    let mut widgets = [
        Widget::new(
            TABS,
            tabs.menu_rect(screen.width),
            String::new(),
            WidgetKind::TabBar(tabs),
        ),
        text(NAME_LABEL, name_label_at, name_label),
        edit_box(NAME, name, Some(language.get("selectWorld.newWorld"))),
        cycle(
            GAME_MODE,
            game_mode,
            "selectWorld.gameMode",
            language.get(&mode_key),
            CycleButton::new(GAME_MODES.len(), mode_index),
        ),
        cycle(
            WORLD_TYPE,
            world_type,
            "selectWorld.mapType",
            type_value,
            CycleButton::new(types.len(), type_index),
        ),
        text(SEED_LABEL, seed_label_at, seed_label),
        edit_box(SEED, seed, None),
        button(CREATE, create, "selectWorld.create"),
        button(CANCEL, cancel, "gui.cancel"),
    ];
    for widget in &mut widgets {
        let id = widget.id();
        widget.visible = !TAB_WIDGETS[1].contains(&id);
        if matches!(id, CREATE | CANCEL) {
            widget.tab_group = 1;
        }
    }
    let amplified = preset.is_some_and(|preset| *preset == AMPLIFIED);
    let mut menu = menu_of(screen, input, widgets)?;
    let tooltip = |widget, key: &str| WidgetTooltip {
        widget,
        text: vec![Span::new(language.get(key), WHITE)],
    };
    menu.tooltips = vec![tooltip(GAME_MODE, &format!("{mode_key}.info"))];
    if amplified {
        menu.tooltips
            .push(tooltip(WORLD_TYPE, "generator.minecraft.amplified.info"));
    }
    menu.hints = vec![WidgetHint {
        widget: SEED,
        text: vec![Span::new(
            language.get("selectWorld.seedInfo"),
            EditBoxHint::DEFAULT_COLOR,
        )],
    }];
    Some(menu)
}

fn menu_of(screen: GuiExtent, input: GuiInput, widgets: [Widget; 9]) -> Option<Menu> {
    let mut list = Widgets::new(screen, input);
    for widget in widgets {
        list.push(widget).ok()?;
    }
    let mut menu = Menu::new(list, MenuBackground::Texture);
    menu.separators = Some(MenuSeparators {
        header_height: None,
        footer_height: FOOTER_LINE,
    });
    Some(menu)
}

fn layout(screen: GuiExtent, name_label: GuiExtent, seed_label: GuiExtent) -> Option<[GuiRect; 8]> {
    let labelled = |label, element| {
        [
            Child::new(label, LayoutCell::default()),
            Child::new(element, LayoutCell::default()),
        ]
    };
    let name_column = labelled(name_label, NAME_BOX);
    let name_column = Stack::column(LABEL_GAP, &name_column);
    let seed_column = labelled(seed_label, SEED_BOX);
    let seed_column = Stack::column(LABEL_GAP, &seed_column);
    let cell = |size, cell, row, column, column_span| GridChild {
        child: Child::new(size, cell),
        row,
        column,
        row_span: 1,
        column_span,
    };
    let game = [
        cell(name_column.size(), ACROSS, 0, 0, 1),
        cell(GAME_ROW, ACROSS, 1, 0, 1),
        cell(GAME_ROW, ACROSS, 2, 0, 1),
        cell(GAME_ROW, ACROSS, 3, 0, 1),
        cell(GAME_ROW, ACROSS, 4, 0, 1),
    ];
    let game = Grid {
        row_gap: ROW_GAP,
        column_gap: 0,
        children: &game,
    };
    let world = [
        cell(HALF_ROW, LayoutCell::default(), 0, 0, 1),
        cell(HALF_ROW, LayoutCell::default(), 0, 1, 1),
        cell(seed_column.size(), LayoutCell::default(), 1, 0, 2),
        cell(SWITCHES, LayoutCell::default(), 2, 0, 2),
    ];
    let world = Grid {
        row_gap: ROW_GAP,
        column_gap: WORLD_COLUMN_GAP,
        children: &world,
    };
    let at = TabBar::content_origin(screen, FOOTER_HEIGHT, game.size());
    let mut game = game.rects(at.x, at.y);
    let (name, game_mode) = (game.next()?, game.next()?);
    let mut name = name_column.rects(name.x, name.y);
    let at = TabBar::content_origin(screen, FOOTER_HEIGHT, world.size());
    let mut world = world.rects(at.x, at.y);
    let (world_type, seed) = (world.next()?, world.nth(1)?);
    let mut seed = seed_column.rects(seed.x, seed.y);
    let footer = [Child::new(FOOTER_BUTTON, LayoutCell::default()); 2];
    let mut footer = Stack::row(FOOTER_GAP, &footer).rects(
        (screen.width - 2 * FOOTER_BUTTON.width - FOOTER_GAP) / 2,
        screen.height - FOOTER_ROW_TOP,
    );
    Some([
        name.next()?,
        name.next()?,
        game_mode,
        world_type,
        seed.next()?,
        seed.next()?,
        footer.next()?,
        footer.next()?,
    ])
}
