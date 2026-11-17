use mcrs_voxel::{BlockPos, SectionPos, VerticalGridPolicy, VerticalWindow};

#[test]
fn test_vertical_window_memory_footprint() {
    // Issue #677 requirement: "The window costs at most 16 bytes per connection"
    let size = core::mem::size_of::<VerticalWindow>();
    assert!(
        size <= 16,
        "VerticalWindow size must be at most 16 bytes, was {size}"
    );
    assert_eq!(size, 8, "VerticalWindow is exactly 8 bytes (i32 + u32)");
}

#[test]
fn test_identity_window_never_translates() {
    let window = VerticalWindow::IDENTITY;
    assert!(window.is_identity());
    assert_eq!(window.num_sections(), 0);

    for y in [-2048, -64, 0, 64, 320, 2048, 8192] {
        assert!(window.contains_section(y));
        assert!(window.contains_block_y(y));
        assert_eq!(window.to_window_section_y(y), Some(y));
        assert_eq!(window.from_window_section_y(y), y);
        assert_eq!(window.to_window_block_y(y), Some(y));
        assert_eq!(window.from_window_block_y(y), y);

        let bp = BlockPos::new(100, y, -200);
        assert_eq!(window.translate_block_pos(bp), Some(bp));
        assert_eq!(window.untranslate_block_pos(bp), bp);

        let sp = SectionPos::new(6, y, -12);
        assert_eq!(window.translate_section_pos(sp), Some(sp));
        assert_eq!(window.untranslate_section_pos(sp), sp);
    }
}

#[test]
fn test_tall_dimension_window_mapping_and_round_trip() {
    // 24 sections = 384 blocks (typical Bedrock / standard modern range)
    // First section = 100 (canonical Y 1600 .. 1984)
    let window = VerticalWindow::new(100, 24);
    assert!(!window.is_identity());
    assert_eq!(window.first_section(), 100);
    assert_eq!(window.num_sections(), 24);
    assert_eq!(window.min_block_y(), 1600);
    assert_eq!(window.max_block_y(), 1983);

    // Within bounds
    assert!(window.contains_section(100));
    assert!(window.contains_section(123));
    assert_eq!(window.to_window_section_y(100), Some(0));
    assert_eq!(window.to_window_section_y(123), Some(23));

    assert_eq!(window.from_window_section_y(0), 100);
    assert_eq!(window.from_window_section_y(23), 123);

    assert!(window.contains_block_y(1600));
    assert!(window.contains_block_y(1983));
    assert_eq!(window.to_window_block_y(1600), Some(0));
    assert_eq!(window.to_window_block_y(1983), Some(383));

    assert_eq!(window.from_window_block_y(0), 1600);
    assert_eq!(window.from_window_block_y(383), 1983);

    // Outside bounds
    assert!(!window.contains_section(99));
    assert!(!window.contains_section(124));
    assert_eq!(window.to_window_section_y(99), None);
    assert_eq!(window.to_window_section_y(124), None);

    assert!(!window.contains_block_y(1599));
    assert!(!window.contains_block_y(1984));
    assert_eq!(window.to_window_block_y(1599), None);
    assert_eq!(window.to_window_block_y(1984), None);
}

#[test]
fn test_extreme_coordinates_and_no_panics() {
    let window = VerticalWindow::new(i32::MAX - 10, 20);
    assert!(window.to_window_section_y(i32::MAX).is_some());
    assert!(window.to_window_section_y(i32::MIN).is_none());

    // Check underflow and overflow resilience
    let neg_window = VerticalWindow::new(i32::MIN, 10);
    assert!(neg_window.to_window_section_y(i32::MIN).is_some());
    assert!(neg_window.to_window_section_y(i32::MAX).is_none());

    // Strict contract check: contains_block_y must equal to_window_block_y.is_some()
    for y in [i32::MIN, -100_000, 0, 100_000, i32::MAX] {
        assert_eq!(
            window.contains_block_y(y),
            window.to_window_block_y(y).is_some(),
            "contains_block_y and to_window_block_y must agree at y = {y}"
        );
        assert_eq!(
            neg_window.contains_block_y(y),
            neg_window.to_window_block_y(y).is_some(),
            "contains_block_y and to_window_block_y must agree at y = {y}"
        );
    }
}

#[test]
fn test_zero_grid_step_does_not_panic() {
    let policy = VerticalGridPolicy {
        grid_step: 0,
        margin_sections: 4,
    };
    // Must never divide by zero
    let w = policy.select_window(50, 24);
    assert_eq!(w.num_sections(), 24);
}

#[test]
fn test_large_num_sections_does_not_wrap_negative() {
    let window = VerticalWindow::new(i32::MIN, 3_000_000_000);
    // Section index must never be negative
    if let Some(sec) = window.to_window_section_y(500_000_000) {
        assert!(sec >= 0, "Window section index must be >= 0");
    }
}

#[test]
fn test_grid_policy_coalesces_players_at_same_height() {
    let policy = VerticalGridPolicy::new(8, 4);

    // Two players at nearby heights (section 101 and section 103)
    // with 24-section windows must receive the exact same snapped window
    let window_p1 = policy.select_window(101, 24);
    let window_p2 = policy.select_window(103, 24);

    assert_eq!(
        window_p1, window_p2,
        "Players at nearby heights must share the same grid window for broadcast re-use"
    );

    // Check rebase detection near edges
    let first = window_p1.first_section();
    let last = first + (window_p1.num_sections() as i32) - 1;

    // Inside center: no rebase needed
    let center = (first + last) / 2;
    assert!(!policy.needs_rebase(&window_p1, center));

    // Near lower margin: rebase needed
    assert!(policy.needs_rebase(&window_p1, first + 1));
    // Near upper margin: rebase needed
    assert!(policy.needs_rebase(&window_p1, last - 1));
}
