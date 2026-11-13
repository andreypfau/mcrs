use mcrs_minecraft_inventory_view::ClickSync;
use mcrs_minecraft_inventory_view::StateId;

fn after(steps: u32) -> StateId {
    (0..steps).fold(StateId::INITIAL, |id, _| id.next())
}

#[test]
fn inv_syn_001_state_id_starts_at_zero_and_wraps_after_32767() {
    let cases: [(u32, u16); 4] = [(0, 0), (1, 1), (32767, 32767), (32768, 0)];
    for (steps, expected) in cases {
        assert_eq!(after(steps).get(), expected, "after {steps} packets");
    }
}

#[test]
fn itx_jav_004_click_with_another_state_id_resends_the_full_content() {
    let server = after(5);
    let cases: [(i32, ClickSync); 4] = [
        (5, ClickSync::CompareEntries),
        (4, ClickSync::FullContent),
        (-1, ClickSync::FullContent),
        (5 + 32768, ClickSync::FullContent),
    ];
    for (packet, expected) in cases {
        assert_eq!(
            server.sync_after_click(packet),
            expected,
            "packet state id {packet}"
        );
    }
}
