use mcrs_minecraft_client_widget::{
    Alignment, Child, EqualGaps, Frame, Grid, GridChild, GuiExtent, GuiRect, HeaderContentFooter,
    LayoutCell, Padding, RowFill, Stack,
};

fn child(width: i32, height: i32, cell: LayoutCell) -> Child {
    Child::new(GuiExtent::new(width, height), cell)
}

fn at(row: u16, column: u16, column_span: u16, child: Child) -> GridChild {
    GridChild {
        child,
        row,
        column,
        row_span: 1,
        column_span,
    }
}

#[test]
fn a_cell_truncates_the_offset_across_and_rounds_the_offset_down() {
    let quarter = LayoutCell {
        alignment: Alignment { x: 0.25, y: 0.25 },
        ..LayoutCell::default()
    };
    let padded = LayoutCell {
        padding: Padding {
            left: 4,
            top: 50,
            right: 4,
            bottom: 4,
        },
        alignment: Alignment { x: 0.5, y: 0.0 },
    };
    let cases = [
        (
            LayoutCell::CENTER,
            GuiExtent::new(10, 10),
            GuiRect::new(100, 200, 23, 23),
            GuiRect::new(106, 207, 10, 10),
        ),
        (
            LayoutCell::CENTER,
            GuiExtent::new(23, 23),
            GuiRect::new(0, 0, 10, 10),
            GuiRect::new(-6, -6, 23, 23),
        ),
        (
            quarter,
            GuiExtent::new(23, 23),
            GuiRect::new(0, 0, 10, 10),
            GuiRect::new(-3, -3, 23, 23),
        ),
        (
            padded,
            GuiExtent::new(150, 20),
            GuiRect::new(0, 0, 158, 74),
            GuiRect::new(4, 50, 150, 20),
        ),
    ];
    for (cell, size, area, expected) in cases {
        assert_eq!(
            cell.place(size, area),
            expected,
            "{size:?} in {area:?} by {cell:?}"
        );
    }
}

#[test]
fn align_truncates_the_offset_on_both_axes() {
    let screen = GuiRect::new(0, 0, 427, 240);
    let cases = [
        (
            screen,
            GuiExtent::new(316, 120),
            Alignment::CENTER,
            GuiRect::new(55, 60, 316, 120),
        ),
        (
            screen,
            GuiExtent::new(212, 118),
            Alignment { x: 0.5, y: 0.25 },
            GuiRect::new(107, 30, 212, 118),
        ),
        (
            GuiRect::new(100, 200, 23, 23),
            GuiExtent::new(10, 10),
            Alignment::CENTER,
            GuiRect::new(106, 206, 10, 10),
        ),
    ];
    for (area, size, alignment, expected) in cases {
        assert_eq!(
            area.align(size, alignment),
            expected,
            "{size:?} in {area:?} at {alignment:?}"
        );
    }
}

#[test]
fn a_grid_sizes_its_tracks_by_the_even_split_of_each_cell() {
    let button = LayoutCell {
        padding: Padding {
            left: 4,
            top: 0,
            right: 4,
            bottom: 4,
        },
        alignment: Alignment { x: 0.5, y: 0.0 },
    };
    let buttons: Vec<GridChild> = (0..10)
        .map(|index| at(index / 2, index % 2, 1, child(150, 20, button)))
        .collect();
    let button_rects: Vec<GuiRect> = (0..10)
        .map(|index| {
            let x = if index % 2 == 0 { 14 } else { 172 };
            GuiRect::new(x, 20 + 24 * (index / 2), 150, 20)
        })
        .collect();
    let top_and_bottom = [
        at(0, 0, 1, child(10, 10, LayoutCell::default())),
        at(2, 0, 1, child(10, 10, LayoutCell::default())),
    ];
    let right = LayoutCell {
        alignment: Alignment { x: 1.0, y: 0.0 },
        ..LayoutCell::default()
    };
    let spanning = [
        at(0, 0, 3, child(9, 10, right)),
        at(1, 2, 1, child(5, 10, LayoutCell::default())),
    ];
    let cases = [
        (
            Grid {
                row_gap: 0,
                column_gap: 0,
                children: &buttons,
            },
            GuiExtent::new(316, 120),
            button_rects,
        ),
        (Grid::default(), GuiExtent::new(0, 0), Vec::new()),
        (
            Grid {
                row_gap: 5,
                column_gap: 0,
                children: &top_and_bottom,
            },
            GuiExtent::new(10, 30),
            vec![GuiRect::new(10, 20, 10, 10), GuiRect::new(10, 40, 10, 10)],
        ),
        (
            Grid {
                row_gap: 0,
                column_gap: 1,
                children: &spanning,
            },
            GuiExtent::new(11, 20),
            vec![GuiRect::new(12, 20, 9, 10), GuiRect::new(16, 30, 5, 10)],
        ),
    ];
    for (grid, size, rects) in cases {
        assert_eq!(grid.size(), size, "{grid:?}");
        assert_eq!(grid.rects(10, 20).collect::<Vec<_>>(), rects, "{grid:?}");
    }
}

#[test]
fn row_fill_moves_a_child_that_does_not_fit_to_the_next_row() {
    let cases: [(u16, &[u16], &[(u16, u16)]); 4] = [
        (2, &[2, 1, 1], &[(0, 0), (1, 0), (1, 1)]),
        (2, &[2, 1, 2], &[(0, 0), (1, 0), (2, 0)]),
        (2, &[1, 2], &[(0, 0), (1, 0)]),
        (3, &[1, 1, 1, 1], &[(0, 0), (0, 1), (0, 2), (1, 0)]),
    ];
    for (columns, spans, expected) in cases {
        let mut fill = RowFill::new(columns);
        let slots: Vec<(u16, u16)> = spans
            .iter()
            .map(|&span| {
                let placed = fill.add(Child::default(), span);
                (placed.row, placed.column)
            })
            .collect();
        assert_eq!(slots, expected, "{columns} columns, spans {spans:?}");
    }
}

#[test]
fn a_stack_adds_one_gap_between_neighbours() {
    let cell = LayoutCell::default();
    let buttons = [child(150, 20, cell), child(150, 30, cell)];
    let spacer = [child(150, 20, cell), child(10, 0, cell)];
    let labelled = [child(40, 9, cell), child(150, 20, cell)];
    let cases = [
        (
            Stack::row(8, &buttons),
            GuiExtent::new(308, 30),
            vec![GuiRect::new(0, 0, 150, 20), GuiRect::new(158, 0, 150, 30)],
        ),
        (
            Stack::row(8, &spacer),
            GuiExtent::new(168, 20),
            vec![GuiRect::new(0, 0, 150, 20), GuiRect::new(158, 0, 10, 0)],
        ),
        (
            Stack::column(4, &labelled),
            GuiExtent::new(150, 33),
            vec![GuiRect::new(0, 0, 40, 9), GuiRect::new(0, 13, 150, 20)],
        ),
    ];
    for (stack, size, rects) in cases {
        assert_eq!(stack.size(), size, "{stack:?}");
        assert_eq!(stack.rects(0, 0).collect::<Vec<_>>(), rects, "{stack:?}");
    }
}

#[test]
fn a_frame_is_as_large_as_its_minimum_and_its_cells() {
    let title = [child(52, 9, LayoutCell::CENTER)];
    let crossed = [
        child(20, 4, LayoutCell::CENTER),
        child(4, 30, LayoutCell::CENTER),
    ];
    let cases = [
        (
            Frame {
                min_size: GuiExtent::new(427, 33),
                children: &title,
            },
            GuiExtent::new(427, 33),
            vec![GuiRect::new(190, 16, 52, 9)],
        ),
        (
            Frame {
                min_size: GuiExtent::new(10, 10),
                children: &crossed,
            },
            GuiExtent::new(20, 30),
            vec![GuiRect::new(3, 17, 20, 4), GuiRect::new(11, 4, 4, 30)],
        ),
        (
            Frame {
                min_size: GuiExtent::new(5, 6),
                children: &[],
            },
            GuiExtent::new(5, 6),
            Vec::new(),
        ),
    ];
    for (frame, size, rects) in cases {
        assert_eq!(frame.size(), size, "{frame:?}");
        assert_eq!(frame.rects(3, 4).collect::<Vec<_>>(), rects, "{frame:?}");
    }
}

#[test]
fn equal_gaps_split_the_free_length_between_neighbours() {
    let square = child(10, 10, LayoutCell::default());
    let four = [square; 4];
    let two = [square; 2];
    let shifted = LayoutCell {
        padding: Padding {
            left: 2,
            ..Padding::default()
        },
        alignment: Alignment { x: 1.0, y: 0.5 },
    };
    let padded = [child(10, 20, shifted); 2];
    let cases = [
        (
            EqualGaps::row(47, 0, &four),
            GuiExtent::new(47, 10),
            vec![
                GuiRect::new(0, 0, 10, 10),
                GuiRect::new(12, 0, 10, 10),
                GuiRect::new(24, 0, 10, 10),
                GuiRect::new(37, 0, 10, 10),
            ],
        ),
        (
            EqualGaps::row(100, 30, &padded),
            GuiExtent::new(100, 30),
            vec![GuiRect::new(2, 5, 10, 20), GuiRect::new(90, 5, 10, 20)],
        ),
        (
            EqualGaps::row(15, 0, &two),
            GuiExtent::new(15, 10),
            vec![GuiRect::new(0, 0, 10, 10), GuiRect::new(5, 0, 10, 10)],
        ),
        (
            EqualGaps::column(50, 0, &two),
            GuiExtent::new(10, 50),
            vec![GuiRect::new(0, 0, 10, 10), GuiRect::new(0, 40, 10, 10)],
        ),
    ];
    for (layout, size, rects) in cases {
        assert_eq!(layout.size(), size, "{layout:?}");
        assert_eq!(layout.rects(0, 0).collect::<Vec<_>>(), rects, "{layout:?}");
    }
}

#[test]
fn the_content_sits_below_the_header_unless_the_footer_pushes_it_up() {
    let screen = GuiExtent::new(427, 240);
    let title = [child(52, 9, LayoutCell::CENTER)];
    let high_header = [child(10, 50, LayoutCell::CENTER)];
    let grid = [child(316, 120, LayoutCell::CENTER)];
    let high_content = [child(316, 200, LayoutCell::CENTER)];
    let done = [child(200, 20, LayoutCell::CENTER)];
    let parts = HeaderContentFooter {
        header: &title,
        content: &grid,
        footer: &done,
        ..HeaderContentFooter::default()
    };
    let cases = [
        (
            parts,
            [
                GuiRect::new(187, 12, 52, 9),
                GuiRect::new(55, 63, 316, 120),
                GuiRect::new(113, 214, 200, 20),
            ],
        ),
        (
            HeaderContentFooter {
                header_height: 61,
                ..parts
            },
            [
                GuiRect::new(187, 26, 52, 9),
                GuiRect::new(55, 87, 316, 120),
                GuiRect::new(113, 214, 200, 20),
            ],
        ),
        (
            HeaderContentFooter {
                content: &high_content,
                ..parts
            },
            [
                GuiRect::new(187, 12, 52, 9),
                GuiRect::new(55, 7, 316, 200),
                GuiRect::new(113, 214, 200, 20),
            ],
        ),
        (
            HeaderContentFooter {
                header: &high_header,
                ..parts
            },
            [
                GuiRect::new(208, 0, 10, 50),
                GuiRect::new(55, 63, 316, 120),
                GuiRect::new(113, 214, 200, 20),
            ],
        ),
    ];
    for (layout, rects) in cases {
        assert_eq!(
            layout.rects(screen).collect::<Vec<_>>(),
            rects,
            "{layout:?}"
        );
    }
}
