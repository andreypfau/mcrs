use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_fixtures::fixture;
use serde::Deserialize;

#[derive(Deserialize)]
struct Types {
    types: Vec<Type>,
}

#[derive(Deserialize)]
struct Type {
    id: String,
    width: f32,
    height: f32,
    eye_height: f32,
    update_interval: u32,
    tracking_range_chunks: u32,
    fire_immune: bool,
}

#[test]
fn each_row_matches_the_type_artefact() {
    let text = fixture("entity_types").text();
    let artefact: Types = serde_json::from_str(&text).expect("the type artefact is JSON");
    for row in EntityTypeData::ALL {
        let id = row.entity_type.as_static_str();
        let Some(expected) = artefact.types.iter().find(|entry| entry.id == id) else {
            panic!("the type artefact has no {id}");
        };
        assert_eq!(row.width, expected.width, "{id} width");
        assert_eq!(row.height, expected.height, "{id} height");
        assert_eq!(row.eye_height, expected.eye_height, "{id} eye height");
        assert_eq!(
            row.update_interval, expected.update_interval,
            "{id} update interval"
        );
        assert_eq!(
            row.tracking_range, expected.tracking_range_chunks,
            "{id} tracking range"
        );
        assert_eq!(row.fire_immune, expected.fire_immune, "{id} fire immune");
    }
}
