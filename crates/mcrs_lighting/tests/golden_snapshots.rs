#![cfg(feature = "test-bench")]

use mcrs_lighting::stub::{block_light_nibbles, sky_light_nibbles};
use mcrs_lighting::test_bench::{assert_nibbles_eq, from_input};

#[path = "golden/mod.rs"]
mod golden;

#[test]
#[ignore = "block-light engine not yet implemented"]
fn snapshot_single_torch() {
    let palette = from_input(golden::single_torch::INPUT);
    let actual = block_light_nibbles(&palette);
    assert_nibbles_eq(&actual, &golden::single_torch::EXPECTED_BLOCK_LIGHT, "single_torch");
}

#[test]
#[ignore = "block-light engine not yet implemented"]
fn snapshot_two_torches_one_removed() {
    let palette = from_input(golden::two_torches_one_removed::INPUT);
    let actual = block_light_nibbles(&palette);
    assert_nibbles_eq(&actual, &golden::two_torches_one_removed::EXPECTED_BLOCK_LIGHT, "two_torches_one_removed");
}

#[test]
#[ignore = "block-light engine not yet implemented"]
fn snapshot_cross_section_horizontal() {
    let palette = from_input(golden::cross_section_horizontal::INPUT);
    let actual = block_light_nibbles(&palette);
    assert_nibbles_eq(&actual, &golden::cross_section_horizontal::EXPECTED_BLOCK_LIGHT, "cross_section_horizontal");
}

#[test]
#[ignore = "block-light engine not yet implemented"]
fn snapshot_vertical_y_boundary() {
    let palette = from_input(golden::vertical_y_boundary::INPUT);
    let actual = block_light_nibbles(&palette);
    assert_nibbles_eq(&actual, &golden::vertical_y_boundary::EXPECTED_BLOCK_LIGHT, "vertical_y_boundary");
}

#[test]
#[ignore = "sky-light engine not yet implemented"]
fn snapshot_empty_sky_above_heightmap() {
    let palette = from_input(golden::empty_sky_above_heightmap::INPUT);
    let actual = sky_light_nibbles(&palette);
    assert_nibbles_eq(&actual, &golden::empty_sky_above_heightmap::EXPECTED_SKY_LIGHT, "empty_sky_above_heightmap");
}

#[test]
#[ignore = "sky-light engine not yet implemented"]
fn snapshot_heightmap_update_on_place() {
    let palette = from_input(golden::heightmap_update_on_place::INPUT);
    let actual = sky_light_nibbles(&palette);
    assert_nibbles_eq(&actual, &golden::heightmap_update_on_place::EXPECTED_SKY_LIGHT, "heightmap_update_on_place");
}

#[test]
#[ignore = "sky-light engine not yet implemented"]
fn snapshot_heightmap_update_on_break() {
    let palette = from_input(golden::heightmap_update_on_break::INPUT);
    let actual = sky_light_nibbles(&palette);
    assert_nibbles_eq(&actual, &golden::heightmap_update_on_break::EXPECTED_SKY_LIGHT, "heightmap_update_on_break");
}

#[test]
#[ignore = "sky-light attenuation engine not yet implemented"]
fn snapshot_sky_attenuation_through_water() {
    let palette = from_input(golden::sky_attenuation_through_water::INPUT);
    let actual = sky_light_nibbles(&palette);
    assert_nibbles_eq(&actual, &golden::sky_attenuation_through_water::EXPECTED_SKY_LIGHT, "sky_attenuation_through_water");
}
