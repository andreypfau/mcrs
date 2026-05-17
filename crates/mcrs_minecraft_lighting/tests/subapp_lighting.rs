// Stub integration test for hosting `LightingPlugin` inside a per-dimension
// SubApp. The body intentionally panics so the test links and runs red until
// the SubApp construction path lands.

use mcrs_engine::world::sub_app::DimAppLabel;

#[test]
fn lighting_plugin_in_subapp() {
    let _ = std::any::type_name::<DimAppLabel>();
    panic!("pending spawn-pump implementation");
}
