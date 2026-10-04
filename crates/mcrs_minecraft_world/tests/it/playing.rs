use crate::common::run_to_playing;
use crate::{
    biome_tags, registry_values, structure_assets, tag_pipeline, timeline_pipeline,
    world_registry_ids,
};

#[test]
fn the_data_pack_reaches_playing_with_every_registry_in_place() {
    let app = run_to_playing();

    biome_tags::the_shipped_biome_tags_resolve(&app);
    biome_tags::the_biome_index_and_snapshot_agree_on_the_id_space(&app);
    registry_values::the_synced_values_differ_from_the_game_as_recorded(&app);
    structure_assets::the_structure_registries_land_before_playing(&app);
    tag_pipeline::tags_load_resolve_and_freeze_on_the_way_to_playing(&app);
    tag_pipeline::entity_type_tags_are_numbered_by_the_report(&app);
    timeline_pipeline::the_timeline_tags_resolve_through_universal(&app);
    timeline_pipeline::every_dimension_builds_its_environment_from_its_tag(&app);
    timeline_pipeline::the_shipped_time_markers_reach_the_overworld_clock(&app);
    timeline_pipeline::the_dimension_timelines_tag_round_trips_to_the_string_the_asset_holds(&app);
    world_registry_ids::the_world_registry_ids_match_the_recorded_fixture(&app);
}
