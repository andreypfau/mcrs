use mcrs_minecraft_client_widget::{
    Alignment, BUTTON_GAP, BUTTON_HEIGHT, BUTTON_WIDTH, Child, DONE_BUTTON_WIDTH, Grid, GuiExtent,
    GuiRect, HeaderContentFooter, LINE_HEIGHT, LayoutCell, Padding, RowFill, Stack,
};

const SCREEN: GuiExtent = GuiExtent::new(427, 240);

#[test]
fn pause_screen_subset_places_its_three_buttons() {
    let cell = LayoutCell {
        padding: Padding {
            left: 4,
            top: 4,
            right: 4,
            bottom: 0,
        },
        alignment: Alignment::default(),
    };
    let first = LayoutCell {
        padding: Padding {
            top: 50,
            ..cell.padding
        },
        ..cell
    };
    let full = GuiExtent::new(204, BUTTON_HEIGHT);
    let half = GuiExtent::new(98, BUTTON_HEIGHT);
    let mut rows = RowFill::new(2);
    let children = [
        rows.add(Child::new(full, first), 2),
        rows.add(Child::new(half, cell), 1),
        rows.add(Child::new(full, cell), 2),
    ];
    let grid = Grid {
        children: &children,
        ..Grid::default()
    };
    let at = GuiRect::new(0, 0, SCREEN.width, SCREEN.height)
        .align(grid.size(), Alignment { x: 0.5, y: 0.25 });

    assert_eq!(at, GuiRect::new(107, 30, 212, 118));
    assert_eq!(
        grid.rects(at.x, at.y).collect::<Vec<_>>(),
        [
            GuiRect::new(111, 80, 204, 20),
            GuiRect::new(111, 104, 98, 20),
            GuiRect::new(111, 128, 204, 20),
        ]
    );
}

#[test]
fn options_screen_subset_places_its_header_grid_and_footer() {
    let title_width = 40;
    let button = GuiExtent::new(BUTTON_WIDTH, BUTTON_HEIGHT);
    let slider = [Child::new(button, LayoutCell::default())];
    let row = Stack::row(BUTTON_GAP, &slider);
    let header_column = [
        Child::new(
            GuiExtent::new(title_width, LINE_HEIGHT),
            LayoutCell {
                alignment: Alignment { x: 0.5, y: 0.0 },
                ..LayoutCell::default()
            },
        ),
        Child::new(row.size(), LayoutCell::default()),
    ];
    let column = Stack::column(BUTTON_GAP, &header_column);
    let grid_cell = LayoutCell {
        padding: Padding {
            left: 4,
            top: 0,
            right: 4,
            bottom: 4,
        },
        alignment: Alignment { x: 0.5, y: 0.0 },
    };
    let mut rows = RowFill::new(2);
    let buttons = [
        rows.add(Child::new(button, grid_cell), 1),
        rows.add(Child::new(button, grid_cell), 1),
    ];
    let grid = Grid {
        children: &buttons,
        ..Grid::default()
    };
    let header = [Child::new(column.size(), LayoutCell::CENTER)];
    let content = [Child::new(grid.size(), LayoutCell::CENTER)];
    let footer = [Child::new(
        GuiExtent::new(DONE_BUTTON_WIDTH, BUTTON_HEIGHT),
        LayoutCell::CENTER,
    )];
    let parts = HeaderContentFooter {
        header_height: 61,
        header: &header,
        content: &content,
        footer: &footer,
        ..HeaderContentFooter::default()
    };
    let [column_at, grid_at, done_at] = parts.rects(SCREEN).collect::<Vec<_>>()[..] else {
        panic!("three parts give three rectangles");
    };
    let [title_at, row_at] = column.rects(column_at.x, column_at.y).collect::<Vec<_>>()[..] else {
        panic!("the header column holds two children");
    };

    assert_eq!(column_at, GuiRect::new(138, 12, 150, 37));
    assert_eq!(title_at, GuiRect::new(193, 12, title_width, 9));
    assert_eq!(
        row.rects(row_at.x, row_at.y).collect::<Vec<_>>(),
        [GuiRect::new(138, 29, 150, 20)]
    );
    assert_eq!(grid_at, GuiRect::new(55, 91, 316, 24));
    assert_eq!(
        grid.rects(grid_at.x, grid_at.y).collect::<Vec<_>>(),
        [
            GuiRect::new(59, 91, 150, 20),
            GuiRect::new(217, 91, 150, 20)
        ]
    );
    assert_eq!(done_at, GuiRect::new(113, 214, 200, 20));
}
