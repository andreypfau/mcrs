use mcrs_minecraft_profile::{
    Profile, ProfileError, ProfileIdentity, Property, SkinPatch, grouped_by_name,
};
use mcrs_minecraft_wire::Encode;

fn with_properties(properties: Vec<Property>) -> Profile {
    Profile {
        profile: ProfileIdentity::Partial {
            name: None,
            id: None,
            properties,
        },
        skin: SkinPatch::default(),
    }
}

#[test]
fn a_profile_with_properties_out_of_their_groups_has_no_saved_or_wire_form() {
    let property = |name: &str, value: &str| Property {
        name: name.into(),
        value: value.into(),
        signature: None,
    };
    let input = vec![property("x", "1"), property("t", "1"), property("x", "2")];
    let ungrouped = with_properties(input.clone());
    let grouped = with_properties(grouped_by_name(input));

    assert_eq!(ungrouped.check(), Err(ProfileError::UngroupedProperties));
    assert!(serde_json::to_string(&ungrouped).is_err());
    assert!(ungrouped.encode(&mut Vec::new()).is_err());

    assert_eq!(grouped, ungrouped);
    assert_eq!(
        serde_json::to_string(&grouped).expect("a grouped profile has a saved form"),
        r#"{"properties":[{"name":"x","value":"1"},{"name":"x","value":"2"},{"name":"t","value":"1"}]}"#
    );
    assert!(grouped.encode(&mut Vec::new()).is_ok());
}
