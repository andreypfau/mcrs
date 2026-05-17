// Stub integration tests for the per-dimension SubApp lifecycle. Bodies are
// intentionally `panic!` so the tests link and run red until the spawn/despawn
// machinery they exercise lands.

use mcrs_engine::world::sub_app::{DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};

#[test]
fn dim_subapp_inserted_on_spawn() {
    let _ = std::any::type_name::<DimAppLabel>();
    panic!("pending spawn-pump implementation");
}

#[test]
fn dim_subapp_removed_on_despawn() {
    let _ = std::any::type_name::<DimDespawnQueue>();
    panic!("pending spawn-pump implementation");
}

#[test]
fn dim_worlds_are_isolated() {
    let _ = std::any::type_name::<DimAppLabel>();
    panic!("pending spawn-pump implementation");
}

#[test]
fn sequential_pump_tick_count() {
    let _ = std::any::type_name::<DimAppLabel>();
    panic!("pending spawn-pump implementation");
}

#[test]
fn no_per_dim_task_pool() {
    let _ = std::any::type_name::<DimAppLabel>();
    panic!("pending spawn-pump implementation");
}

#[test]
fn registries_present_in_all_subapps() {
    let _ = std::any::type_name::<DimSpawnRequest>();
    panic!("pending spawn-pump implementation");
}

#[test]
fn time_extracted_into_subapp() {
    let _ = std::any::type_name::<DimAppLabel>();
    panic!("pending spawn-pump implementation");
}

#[test]
fn eager_spawn_count_matches_dims() {
    let _ = std::any::type_name::<DimSpawnQueue>();
    panic!("pending eager spawn implementation");
}
