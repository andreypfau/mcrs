use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::LazyLock;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct TestId {
    pub binary: String,
    pub name: String,
}

static ESCAPES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("\u{1b}\\[[0-9;?]*[A-Za-z]").expect("a valid pattern"));

static STATUS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*(FAIL \+ LEAK|FAIL|LEAK-FAIL|TIMEOUT|SIG[A-Z0-9]+|ABORT|XFAIL)\s+\[\s*[0-9.]+s\]\s+(?:\([^)]*\)\s+)?(\S+)\s+(\S+)\s*$",
    )
    .expect("a valid pattern")
});

fn runs_tests(argv_line: &str) -> bool {
    let words: Vec<&str> = argv_line
        .split_whitespace()
        .map(|word| word.trim_matches(['"', '\'']))
        .collect();
    words.windows(2).any(|pair| pair == ["nextest", "run"]) && !words.contains(&"--no-run")
}

fn test_section(log: &str) -> Option<&str> {
    let mut found = None;
    let mut open = false;
    let mut offset = 0;
    for line in log.split_inclusive('\n') {
        offset += line.len();
        if let Some(argv) = line.strip_prefix("*** ") {
            open = runs_tests(argv);
            if open {
                found = Some((offset, offset));
            }
        } else if open && let Some((start, _)) = found {
            found = Some((start, offset));
        }
    }
    found.map(|(start, end)| &log[start..end])
}

pub(crate) fn failing_tests(log: &str) -> Result<BTreeSet<TestId>, String> {
    let section = test_section(log).ok_or_else(|| {
        "the log has no test run section, so no failing test can be read from it".to_owned()
    })?;
    let mut failing = BTreeSet::new();
    for line in section.lines() {
        let line = ESCAPES.replace_all(line, "");
        let Some(found) = STATUS.captures(&line) else {
            continue;
        };
        let (status, binary, name) = (&found[1], &found[2], &found[3]);
        if status == "XFAIL" {
            return Err(format!(
                "nextest reported XFAIL for {binary} {name}, so its binary did not start; fix the environment and measure again"
            ));
        }
        failing.insert(TestId {
            binary: binary.to_owned(),
            name: name.to_owned(),
        });
    }
    Ok(failing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    const REAL_1: &str = r#"        FAIL [   0.065s] (165/772) mcrs_minecraft_random legacy::test::every_draw_matches_the_java_sequence
        FAIL [   0.085s] (173/772) mcrs_minecraft_random xoroshiro::test::every_draw_matches_the_java_sequence_for_seed_1
        FAIL [   0.046s] (258/772) mcrs_minecraft_worldgen_density node::end_island::tests::matches_the_vanilla_oracle
        FAIL [   0.160s] (410/772) mcrs_minecraft_worldgen_feature_place entity::tests::a_shulker_and_a_frame_draw_nothing_and_a_minecart_draws_its_seed
        FAIL [   0.364s] (411/772) mcrs_minecraft_worldgen_feature_place entity::tests::a_mob_draws_the_follow_range_bonus_then_its_hand
        FAIL [   0.446s] (416/772) mcrs_minecraft_worldgen_feature_place entity::tests::a_vindicator_holds_an_axe_and_rolls_its_enchantment
        FAIL [   0.330s] (417/772) mcrs_minecraft_worldgen_feature_place entity::tests::the_allay_count_is_drawn_first_then_each_allay
        FAIL [   0.799s] (420/772) mcrs_minecraft_worldgen_feature_place entity::tests::a_cat_in_a_swamp_hut_is_all_black_after_one_draw
        FAIL [   0.828s] (423/772) mcrs_minecraft_worldgen_feature_place entity::tests::a_bare_drowned_draws_the_zombie_sequence
        FAIL [   0.401s] (433/772) mcrs_minecraft_worldgen_feature_place geode::tests::geode_draw_count_anchor
        FAIL [   0.015s] (693/772) mcrs_minecraft_worldgen_noise normal::bound_tests::the_modes_sample_apart
        FAIL [  17.266s] (765/772) mcrs_minecraft_worldgen_generator tests::carving_pins::the_carved_column_is_what_it_was
────────────
     Summary [  54.455s] 772 tests run: 760 passed (12 slow), 12 failed, 58 skipped
        FAIL [   0.065s] (165/772) mcrs_minecraft_random legacy::test::every_draw_matches_the_java_sequence
        FAIL [   0.085s] (173/772) mcrs_minecraft_random xoroshiro::test::every_draw_matches_the_java_sequence_for_seed_1
        FAIL [   0.046s] (258/772) mcrs_minecraft_worldgen_density node::end_island::tests::matches_the_vanilla_oracle
        FAIL [   0.160s] (410/772) mcrs_minecraft_worldgen_feature_place entity::tests::a_shulker_and_a_frame_draw_nothing_and_a_minecart_draws_its_seed
        FAIL [   0.364s] (411/772) mcrs_minecraft_worldgen_feature_place entity::tests::a_mob_draws_the_follow_range_bonus_then_its_hand
        FAIL [   0.446s] (416/772) mcrs_minecraft_worldgen_feature_place entity::tests::a_vindicator_holds_an_axe_and_rolls_its_enchantment
        FAIL [   0.330s] (417/772) mcrs_minecraft_worldgen_feature_place entity::tests::the_allay_count_is_drawn_first_then_each_allay
        FAIL [   0.799s] (420/772) mcrs_minecraft_worldgen_feature_place entity::tests::a_cat_in_a_swamp_hut_is_all_black_after_one_draw
        FAIL [   0.828s] (423/772) mcrs_minecraft_worldgen_feature_place entity::tests::a_bare_drowned_draws_the_zombie_sequence
        FAIL [   0.401s] (433/772) mcrs_minecraft_worldgen_feature_place geode::tests::geode_draw_count_anchor
        FAIL [   0.015s] (693/772) mcrs_minecraft_worldgen_noise normal::bound_tests::the_modes_sample_apart
        FAIL [  17.266s] (765/772) mcrs_minecraft_worldgen_generator tests::carving_pins::the_carved_column_is_what_it_was
error: test run failed
"#;

    const REAL_2: &str = r#"────────────
     Summary [  10.145s] 772 tests run: 769 passed, 3 failed, 58 skipped
        FAIL [   0.008s] (167/772) mcrs_minecraft_random legacy::test::next_gaussian_banks_the_second_of_the_polar_pair
        FAIL [   0.007s] (175/772) mcrs_minecraft_random xoroshiro::test::next_gaussian_banks_the_second_of_the_polar_pair
        FAIL [   0.009s] (190/772) mcrs_minecraft_value_provider tests::clamped_normal_spends_one_gaussian
error: test run failed
"#;

    const REAL_3: &str = r#"────────────
     Summary [  65.304s] 772 tests run: 703 passed, 64 failed, 5 timed out, 58 skipped
        FAIL [   1.703s] (122/772) mcrs_minecraft_client blocks::tint::tests::jar::biome_tints_read_the_resource_pack
        FAIL [   0.008s] (165/772) mcrs_minecraft_random legacy::test::every_draw_matches_the_java_sequence
        FAIL [   0.009s] (167/772) mcrs_minecraft_random test::a_legacy_positional_factory_matches_java
        FAIL [   0.019s] (169/772) mcrs_minecraft_random legacy::test::next_gaussian_banks_the_second_of_the_polar_pair
        FAIL [   0.009s] (200/772) mcrs_minecraft_worldgen_carver canyon::tests::a_canyon_marks_cells_inside_the_target_chunk
        FAIL [   0.012s] (201/772) mcrs_minecraft_worldgen_carver canyon::tests::the_centre_factor_widens_the_middle_of_the_run
        FAIL [   0.011s] (203/772) mcrs_minecraft_worldgen_carver canyon::tests::the_cross_section_stretches_further_than_an_ellipsoid
        FAIL [   0.011s] (204/772) mcrs_minecraft_worldgen_carver canyon::tests::the_width_table_covers_the_dimension_and_holds_squares
        FAIL [   0.013s] (209/772) mcrs_minecraft_worldgen_carver modern::tests::the_modern_caves_walk_on_the_modern_sine_index
        FAIL [   0.009s] (215/772) mcrs_minecraft_worldgen_carver target::tests::a_source_past_a_columns_radius_does_not_carve_it
        FAIL [   0.011s] (220/772) mcrs_minecraft_worldgen_density beta::tests::beta_climate_postprocess_values_match_fixture
        FAIL [   0.010s] (221/772) mcrs_minecraft_worldgen_density beta::tests::beta_seeding_no_discard_draw_count
        FAIL [   0.010s] (222/772) mcrs_minecraft_worldgen_density beta::tests::the_dropped_octaves_still_position_the_ones_we_keep
        FAIL [   0.071s] (224/772) mcrs_minecraft_worldgen_carver target::tests::a_region_slot_equals_its_columns_mask
        FAIL [   0.125s] (246/772) mcrs_minecraft_worldgen_density compile::tests::the_end_final_density_is_a_real_field
        FAIL [   0.124s] (247/772) mcrs_minecraft_worldgen_density compile::tests::the_end_islands_slice_pins_y_to_zero
        FAIL [   0.009s] (256/772) mcrs_minecraft_worldgen_density node::end_island::tests::matches_the_vanilla_oracle
        FAIL [   0.009s] (257/772) mcrs_minecraft_worldgen_density node::end_island::tests::the_section_division_truncates_towards_zero
        FAIL [   0.011s] (258/772) mcrs_minecraft_worldgen_density node::end_island::tests::the_extent_holds_one_value_per_column_in_layout_order
        FAIL [   0.012s] (263/772) mcrs_minecraft_worldgen_density node::end_island::tests::the_value_ignores_y
        FAIL [   0.009s] (342/772) mcrs_minecraft_worldgen_feature placer::tests::the_count_noise_is_the_fixed_field
        FAIL [   0.009s] (408/772) mcrs_minecraft_worldgen_feature_place end::tests::the_seeded_spikes_lie_on_the_radius_forty_two_ring
        FAIL [   0.009s] (421/772) mcrs_minecraft_worldgen_feature_place geode::tests::an_invalid_point_ends_the_run_before_its_offset
        FAIL [   0.013s] (423/772) mcrs_minecraft_worldgen_feature_place geode::tests::crystals_only_grow_where_the_alternate_layer_went_in
        FAIL [   0.014s] (424/772) mcrs_minecraft_worldgen_feature_place geode::tests::geode_draw_count_anchor
        FAIL [   0.010s] (514/772) mcrs_minecraft_worldgen_feature_place template::tests::capped_replaces_at_most_the_limit_in_shuffled_order
        FAIL [   0.009s] (518/772) mcrs_minecraft_worldgen_feature_place template::tests::the_clip_bounds_the_writes_with_and_without_a_capped_chain
        FAIL [   0.008s] (525/772) mcrs_minecraft_worldgen_feature_place terrain_skin::tests::height_cools_a_column_above_the_snow_line
        FAIL [   0.011s] (548/772) mcrs_minecraft_worldgen_feature_place tree::provider::tests::noise_threshold_spends_one_draw_below_and_two_above
        FAIL [   0.009s] (553/772) mcrs_minecraft_worldgen_feature_place tree::provider::tests::the_noise_providers_draw_nothing
        FAIL [   0.489s] (598/772) mcrs_minecraft_worldgen_generator stages::tests::the_writer_keeps_the_centre_and_routes_the_ring
        FAIL [   0.497s] (599/772) mcrs_minecraft_worldgen_generator stages::tests::an_entity_reaches_the_column_under_it_and_merges_in_once
        FAIL [   0.459s] (601/772) mcrs_minecraft_worldgen_generator tests::beta_biome_grid::every_cell_is_the_biome_its_climate_answers_and_a_ring_cell_is_the_neighbour_s
        FAIL [   0.462s] (602/772) mcrs_minecraft_worldgen_generator tests::beta_ore_distribution::the_populate_feature_is_the_populate_step
        FAIL [   0.367s] (605/772) mcrs_minecraft_worldgen_generator tests::beta_surface::beta_surface_bedrock_matches_back2beta_oracle
        FAIL [   0.387s] (606/772) mcrs_minecraft_worldgen_generator tests::beta_surface::beta_terrain_height_matches_back2beta_oracle
        FAIL [   0.386s] (609/772) mcrs_minecraft_worldgen_generator tests::carving_regions::a_region_slot_equals_the_column_carved_alone
        FAIL [   0.419s] (612/772) mcrs_minecraft_worldgen_generator tests::corpus_generators::every_feature_and_every_feature_type_of_the_corpus_has_a_generator
        FAIL [   0.378s] (615/772) mcrs_minecraft_worldgen_generator tests::footprint::a_read_past_the_ring_is_a_debug_assertion
        FAIL [   0.463s] (616/772) mcrs_minecraft_worldgen_generator tests::corpus_generators::the_corpus_features_compile_to_their_shapes
        FAIL [   0.509s] (618/772) mcrs_minecraft_worldgen_generator tests::corpus_ores::the_corpus_ore_counts_stay_in_the_band_the_first_run_set
        FAIL [   0.399s] (619/772) mcrs_minecraft_worldgen_generator tests::footprint::a_write_past_the_ring_is_a_debug_assertion
        FAIL [   0.386s] (620/772) mcrs_minecraft_worldgen_generator tests::footprint::the_corners_of_the_ring_are_inside_the_footprint
        FAIL [   0.360s] (621/772) mcrs_minecraft_worldgen_generator tests::modern_carvers::a_beta_source_runs_the_carvers_of_its_palette_biome
        FAIL [   1.194s] (622/772) mcrs_minecraft_worldgen_generator tests::carving_regions::a_columns_blocks_do_not_depend_on_the_region_width
        FAIL [   0.362s] (626/772) mcrs_minecraft_worldgen_generator tests::modern_features::a_modern_run_places_the_ore_the_window_biome_carries
        FAIL [   1.556s] (633/772) mcrs_minecraft_worldgen_generator tests::carving_pins::the_carved_column_is_what_it_was
        FAIL [   0.767s] (639/772) mcrs_minecraft_worldgen_generator tests::modern_carvers::carving_an_overworld_column_frees_space_and_spares_bedrock
        FAIL [   0.499s] (643/772) mcrs_minecraft_worldgen_generator tests::modern_features::a_nested_feature_without_a_generator_still_spends_its_chain
        FAIL [   1.911s] (651/772) mcrs_minecraft_worldgen_generator tests::cell_fill::a_bearded_column_matches_the_block_by_block_fill_of_the_summed_density
        FAIL [   0.410s] (661/772) mcrs_minecraft_worldgen_generator tests::support::every_shipped_noise_settings_compiles_its_material_rules
        FAIL [   2.553s] (662/772) mcrs_minecraft_worldgen_generator tests::carving_pins::the_per_column_mask_is_what_it_was
        FAIL [   0.010s] (693/772) mcrs_minecraft_worldgen_noise normal::bound_tests::each_normalization_scales_the_octave_amplitudes
        FAIL [   0.012s] (694/772) mcrs_minecraft_worldgen_noise normal::bound_tests::the_modes_sample_apart
        FAIL [   0.013s] (696/772) mcrs_minecraft_worldgen_noise perlin::modern::column_matches_per_position_bit_for_bit
        FAIL [   0.011s] (697/772) mcrs_minecraft_worldgen_noise perlin::modern::modern_offset_is_vanilla
        FAIL [   0.035s] (711/772) mcrs_minecraft_worldgen_structure locate::tests::a_ring_answer_only_loses_to_a_strictly_closer_spread_hit
        FAIL [   0.037s] (713/772) mcrs_minecraft_worldgen_structure locate::tests::the_first_non_empty_radius_answers_even_when_a_closer_start_lies_beyond_it
        FAIL [   0.011s] (716/772) mcrs_minecraft_worldgen_structure orient::tests::a_random_orientation_reads_the_horizontal_plane_in_face_order
        FAIL [   0.008s] (731/772) mcrs_minecraft_worldgen_structure placement::tests::removal_shifts_the_draw_to_the_remaining_entries
        FAIL [   0.010s] (732/772) mcrs_minecraft_worldgen_structure placement::tests::rings_grow_by_two_thirds_and_the_last_takes_what_remains
        FAIL [   0.024s] (748/772) mcrs_minecraft_worldgen_structure_place nether_fossil::tests::the_dried_ghast_lands_on_the_fossil_floor_when_its_cell_is_air
        FAIL [   0.026s] (749/772) mcrs_minecraft_worldgen_surface compile::tests::the_band_table_matches_the_reference_draw_order
        FAIL [   2.043s] (765/772) mcrs_minecraft_worldgen_generator tests::structure_index::the_index_finds_the_nearest_village_and_stronghold
     TIMEOUT [  60.008s] (768/772) mcrs_minecraft_worldgen_feature_place ore_beta::tests::place_draw_count_anchor
     TIMEOUT [  60.008s] (769/772) mcrs_minecraft_worldgen_feature_place ore_beta::tests::a_vein_sits_above_the_source_it_grew_from
     TIMEOUT [  60.008s] (770/772) mcrs_minecraft_worldgen_feature_place ore_beta::tests::place_only_replaces_stone_target
     TIMEOUT [  60.011s] (771/772) mcrs_minecraft_worldgen_generator beta_ores::tests::clay_starts_only_in_water_and_turns_only_sand
     TIMEOUT [  60.009s] (772/772) mcrs_minecraft_worldgen_generator tests::beta_ore_distribution::veins_cross_the_column_border_on_the_pinned_draw_count
error: test run failed
"#;

    const REAL_4: &str = r#"────────────
     Summary [  10.860s] 772 tests run: 768 passed, 4 failed, 58 skipped
        FAIL [   0.013s] (170/772) mcrs_minecraft_random test::a_legacy_positional_factory_matches_java
        FAIL [   0.011s] (172/772) mcrs_minecraft_random test::block_pos_seed_matches_java
        FAIL [   0.011s] (412/772) mcrs_minecraft_worldgen_feature_place entity::tests::a_shulker_and_a_frame_draw_nothing_and_a_minecart_draws_its_seed
        FAIL [   2.588s] (746/772) mcrs_minecraft_worldgen_generator tests::carving_pins::the_carved_column_is_what_it_was
error: test run failed
"#;

    const REAL_5: &str = r#"────────────
     Summary [   9.808s] 772 tests run: 769 passed, 3 failed, 58 skipped
        FAIL [   0.008s] (169/772) mcrs_minecraft_random test::a_legacy_positional_factory_matches_java
        FAIL [   0.007s] (170/772) mcrs_minecraft_random test::block_pos_seed_matches_java
        FAIL [   2.861s] (764/772) mcrs_minecraft_worldgen_generator tests::carving_pins::the_carved_column_is_what_it_was
error: test run failed
"#;

    const REAL_6: &str = r#"────────────
     Summary [   9.719s] 772 tests run: 751 passed, 21 failed, 58 skipped
        FAIL [   0.009s] (172/772) mcrs_minecraft_random xoroshiro::test::every_draw_matches_the_java_sequence_for_seed_1
        FAIL [   0.016s] (427/772) mcrs_minecraft_worldgen_feature_place geode::tests::geode_draw_count_anchor
        FAIL [   0.008s] (435/772) mcrs_minecraft_worldgen_feature_place iceberg::tests::a_round_iceberg_leaves_a_pinned_source_state
        FAIL [   0.009s] (444/772) mcrs_minecraft_worldgen_feature_place mossy_carpet::tests::the_topper_only_grows_where_the_wall_reaches_above_the_carpet
        FAIL [   0.012s] (468/772) mcrs_minecraft_worldgen_feature_place room::tests::room_draw_count_anchor
        FAIL [   0.012s] (493/772) mcrs_minecraft_worldgen_feature_place speleothem::tests::cluster_draw_count_anchor
        FAIL [   0.009s] (496/772) mcrs_minecraft_worldgen_feature_place spike::tests::a_spike_draws_its_shape_then_its_rim_then_its_roots
        FAIL [   0.011s] (547/772) mcrs_minecraft_worldgen_feature_place tree::foliage::tests::every_foliage_placer_writes_and_draws_what_it_did
        FAIL [   0.010s] (566/772) mcrs_minecraft_worldgen_feature_place tree::trunk::tests::every_trunk_placer_writes_and_draws_what_it_did
        FAIL [   0.012s] (583/772) mcrs_minecraft_worldgen_generator stages::tests::the_decoration_seed_is_the_reference_chain
        FAIL [   0.611s] (610/772) mcrs_minecraft_worldgen_generator tests::base_height::base_height_matches_a_descent_of_the_raw_fill
        FAIL [   0.664s] (623/772) mcrs_minecraft_worldgen_generator tests::corpus_ores::the_corpus_ore_counts_stay_in_the_band_the_first_run_set
        FAIL [   0.467s] (636/772) mcrs_minecraft_worldgen_generator tests::modern_features::a_modern_run_places_the_ore_the_window_biome_carries
        FAIL [   1.535s] (651/772) mcrs_minecraft_worldgen_generator tests::carving_regions::a_columns_blocks_do_not_depend_on_the_region_width
        FAIL [   0.488s] (652/772) mcrs_minecraft_worldgen_generator tests::multi_noise_biomes::the_grid_rings_the_column_by_one_quart_cell
        FAIL [   0.357s] (658/772) mcrs_minecraft_worldgen_generator tests::structure_sites::base_heights_match_the_oracle
        FAIL [   0.420s] (671/772) mcrs_minecraft_worldgen_generator tests::surface::an_overworld_column_gets_grass_over_dirt_over_stone
        FAIL [   0.486s] (697/772) mcrs_minecraft_worldgen_generator tests::surface::bypassing_every_shortcut_writes_the_same_blocks
        FAIL [   2.334s] (763/772) mcrs_minecraft_worldgen_generator tests::cell_fill::a_bearded_column_matches_the_block_by_block_fill_of_the_summed_density
        FAIL [   0.118s] (765/772) mcrs_minecraft_worldgen_surface oracle::tests::the_tape_and_the_oracle_agree_over_the_whole_context_range
        FAIL [   2.729s] (767/772) mcrs_minecraft_worldgen_generator tests::carving_pins::the_carved_column_is_what_it_was
error: test run failed
"#;

    const REAL_7: &str = r#"        FAIL [   0.160s] (371/558) mcrs_minecraft_worldgen_density::it aquifer::a_louder_barrier_widens_the_shell_and_stays_exact
────────────
     Summary [  23.325s] 558 tests run: 557 passed, 1 failed, 54 skipped
        FAIL [   0.160s] (371/558) mcrs_minecraft_worldgen_density::it aquifer::a_louder_barrier_widens_the_shell_and_stays_exact
error: test run failed
"#;

    const REAL_8: &str = r#"────────────
     Summary [  22.785s] 558 tests run: 555 passed (2 leaky), 3 failed, 54 skipped
        FAIL [   0.175s] (373/558) mcrs_minecraft_worldgen_density::it aquifer::a_louder_barrier_widens_the_shell_and_stays_exact
        FAIL [   0.180s] (375/558) mcrs_minecraft_worldgen_density::it aquifer::lemmas_agree_with_the_search_block_for_block
        FAIL [   2.897s] (552/558) mcrs_minecraft_worldgen_generator tests::carving_pins::the_carved_column_is_what_it_was
error: test run failed
"#;

    const REAL_9: &str = r#"────────────
     Summary [  22.844s] 558 tests run: 557 passed (1 leaky), 1 failed, 54 skipped
        FAIL [   0.147s] (371/558) mcrs_minecraft_worldgen_density::it aquifer::a_louder_barrier_widens_the_shell_and_stays_exact
error: test run failed
"#;

    const REAL: [(&str, &str, &str, &str); 9] = [
        (
            "fixture 1",
            REAL_1,
            "mcrs_minecraft_random",
            "legacy::test::every_draw_matches_the_java_sequence",
        ),
        (
            "fixture 2",
            REAL_2,
            "mcrs_minecraft_random",
            "legacy::test::next_gaussian_banks_the_second_of_the_polar_pair",
        ),
        (
            "fixture 3",
            REAL_3,
            "mcrs_minecraft_client",
            "blocks::tint::tests::jar::biome_tints_read_the_resource_pack",
        ),
        (
            "fixture 4",
            REAL_4,
            "mcrs_minecraft_random",
            "test::a_legacy_positional_factory_matches_java",
        ),
        (
            "fixture 5",
            REAL_5,
            "mcrs_minecraft_random",
            "test::a_legacy_positional_factory_matches_java",
        ),
        (
            "fixture 6",
            REAL_6,
            "mcrs_minecraft_random",
            "xoroshiro::test::every_draw_matches_the_java_sequence_for_seed_1",
        ),
        (
            "fixture 7",
            REAL_7,
            "mcrs_minecraft_worldgen_density::it",
            "aquifer::a_louder_barrier_widens_the_shell_and_stays_exact",
        ),
        (
            "fixture 8",
            REAL_8,
            "mcrs_minecraft_worldgen_density::it",
            "aquifer::a_louder_barrier_widens_the_shell_and_stays_exact",
        ),
        (
            "fixture 9",
            REAL_9,
            "mcrs_minecraft_worldgen_density::it",
            "aquifer::a_louder_barrier_widens_the_shell_and_stays_exact",
        ),
    ];

    fn section(lines: &str) -> String {
        format!(
            "*** cargo nextest run --cargo-profile=mutants --profile=mutants-matrix --no-tests=pass\n{lines}\n"
        )
    }

    fn id(binary: &str, name: &str) -> TestId {
        TestId {
            binary: binary.to_owned(),
            name: name.to_owned(),
        }
    }

    fn reported_failures(section: &str) -> usize {
        let count = Regex::new(r"(\d+) (?:failed|timed out)").unwrap();
        section
            .lines()
            .filter(|line| line.contains("Summary ["))
            .flat_map(|line| count.captures_iter(line))
            .map(|caps| caps[1].parse::<usize>().unwrap())
            .sum()
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "real nextest output for each failure shape followed by the rows that parse synthetic lines"
    )]
    fn nextest_failure_lines_name_the_tests_a_mutant_failed() {
        for (name, text, binary, test) in REAL {
            let found = failing_tests(&section(text)).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                found.contains(&id(binary, test)),
                "{name}: {binary} {test} not in {found:?}"
            );
            assert_eq!(
                found.len(),
                reported_failures(text),
                "{name}: the summary line counts the failures"
            );
        }

        type Row = (
            &'static str,
            String,
            Result<Vec<(&'static str, &'static str)>, &'static str>,
        );
        let build_failure = "*** cargo nextest run --no-run --cargo-profile=mutants\n        FAIL [   0.1s] (1/2) pkg stray::in_the_build\n*** result: Failure(101)\n*** cargo nextest run --cargo-profile=mutants\n".to_owned();
        let rows: Vec<Row> = vec![
            (
                "FAIL",
                section("        FAIL [   0.065s] (  9/100) pkg a::b"),
                Ok(vec![("pkg", "a::b")]),
            ),
            (
                "FAIL + LEAK",
                section("  FAIL + LEAK [   1.000s] (  9/100) pkg a::c"),
                Ok(vec![("pkg", "a::c")]),
            ),
            (
                "LEAK-FAIL",
                section("   LEAK-FAIL [   1.000s] (  9/100) pkg a::d"),
                Ok(vec![("pkg", "a::d")]),
            ),
            (
                "TIMEOUT",
                section("     TIMEOUT [  60.000s] ( 13/100) pkg a::e"),
                Ok(vec![("pkg", "a::e")]),
            ),
            (
                "SIGSEGV",
                section("     SIGSEGV [   1.000s] ( 14/100) pkg a::f"),
                Ok(vec![("pkg", "a::f")]),
            ),
            (
                "ABORT",
                section("       ABORT [   1.000s] ( 15/100) pkg a::g"),
                Ok(vec![("pkg", "a::g")]),
            ),
            (
                "XFAIL is an infrastructure error",
                section("       XFAIL [   0.000s] ( 16/100) pkg a::h"),
                Err("XFAIL"),
            ),
            (
                "a passing test",
                section("        PASS [   0.010s] ( 17/100) pkg a::i"),
                Ok(vec![]),
            ),
            (
                "a started test",
                section("       START [         ] (  1/772) pkg a::l"),
                Ok(vec![]),
            ),
            (
                "a slow test",
                section("        SLOW [> 15.000s] (  1/772) pkg a::m"),
                Ok(vec![]),
            ),
            (
                "ANSI escapes",
                section(
                    "\u{1b}[31;1m        FAIL\u{1b}[0m [\u{1b}[1m   0.065s\u{1b}[0m] (  9/100) \u{1b}[35mpkg\u{1b}[0m a::j",
                ),
                Ok(vec![("pkg", "a::j")]),
            ),
            (
                "a final-section repeat padded with the bar character",
                section(
                    "        FAIL [   0.065s] (  9/100) pkg a::k\n────────────\n     Summary [   1.0s] 100 tests run: 99 passed, 1 failed\n        FAIL [   0.065s] (─────────) pkg a::k",
                ),
                Ok(vec![("pkg", "a::k")]),
            ),
            (
                "an integration binary id",
                section(
                    "        FAIL [   0.065s] (  9/100) pkg::it a::n\n        FAIL [   0.065s] (  9/100) pkg::bin/tool a::o",
                ),
                Ok(vec![("pkg::bin/tool", "a::o"), ("pkg::it", "a::n")]),
            ),
            (
                "the result line that ends the test section",
                format!(
                    "{}*** result: Failure(100)\n\n",
                    section("        FAIL [   0.006s] (  3/3) pkg a::p")
                ),
                Ok(vec![("pkg", "a::p")]),
            ),
            (
                "a failure line in the build section only",
                build_failure,
                Ok(vec![]),
            ),
            (
                "a log with no test section",
                "*** cargo nextest run --no-run\n".to_owned(),
                Err("no test run"),
            ),
        ];
        for (name, text, expected) in rows {
            match (expected, failing_tests(&text)) {
                (Ok(pairs), Ok(found)) => {
                    let wanted: BTreeSet<TestId> =
                        pairs.into_iter().map(|(b, t)| id(b, t)).collect();
                    assert_eq!(found, wanted, "{name}");
                }
                (Err(part), Err(message)) => {
                    assert!(message.contains(part), "{name}: {part} not in {message}")
                }
                (expected, found) => panic!("{name}: expected {expected:?}, found {found:?}"),
            }
        }
    }
}
