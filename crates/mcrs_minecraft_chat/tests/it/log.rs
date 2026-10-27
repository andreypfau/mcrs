use mcrs_minecraft_chat::{ChatLog, Overlay, fade};

fn one_line(message: &u32, rows: &mut Vec<u32>) {
    rows.push(*message);
}

#[test]
fn the_log_keeps_the_hundred_newest_lines() {
    let mut log = ChatLog::<u32, u32>::default();
    for message in 1..=150 {
        log.push(message, 0, None, false, one_line);
    }
    assert_eq!(log.lines().len(), 100);
    assert_eq!(log.lines().last().unwrap().content, 51);
    assert_eq!(log.lines().next().unwrap().content, 150);
}

#[test]
fn new_lines_keep_a_scrolled_open_chat_in_place() {
    let mut log = ChatLog::<u32, u32>::default();
    for message in 0..40 {
        log.push(message, 0, None, true, one_line);
    }
    log.scroll_by(19, 20);
    log.push(40, 0, None, true, one_line);
    assert_eq!(log.scroll(), 20);
    assert!(log.new_since_scroll());
    log.scroll_by(-100, 20);
    assert_eq!(log.scroll(), 0);
    assert!(!log.new_since_scroll());
}

#[test]
fn a_line_holds_for_nine_seconds_and_fades_over_the_tenth() {
    for (age, alpha) in [(0, 1.0), (180, 1.0), (190, 0.25), (200, 0.0), (400, 0.0)] {
        assert!((fade(age) - alpha).abs() < 1e-6, "{age}: {}", fade(age));
    }
}

#[test]
fn the_overlay_shows_for_three_seconds_and_fades_in_the_last() {
    let mut overlay = Overlay::default();
    overlay.set("a");
    for _ in 0..40 {
        overlay.tick();
    }
    assert_eq!(overlay.shown(0.0), Some((&"a", 255)));
    for _ in 0..10 {
        overlay.tick();
    }
    assert_eq!(overlay.shown(0.0), Some((&"a", 127)));
    for _ in 0..10 {
        overlay.tick();
    }
    assert_eq!(overlay.shown(0.0), None);
}
