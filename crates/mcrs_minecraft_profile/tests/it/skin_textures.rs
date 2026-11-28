use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use mcrs_minecraft_profile::{PlayerModelType, Property, SkinTextures};

fn textures(json: &str) -> Property {
    Property {
        name: "textures".into(),
        value: STANDARD.encode(json),
        signature: None,
    }
}

#[test]
fn the_textures_property_names_the_skin_and_its_arm_model() {
    let url = "http://textures.minecraft.net/texture/abc";
    let cases = [
        (
            r#"{"textures":{"SKIN":{"url":"http://textures.minecraft.net/texture/abc","metadata":{"model":"slim"}}}}"#,
            Some(PlayerModelType::Slim),
        ),
        (
            r#"{"timestamp":1,"textures":{"SKIN":{"url":"http://textures.minecraft.net/texture/abc"},"CAPE":{"url":"x"}}}"#,
            Some(PlayerModelType::Wide),
        ),
        (r#"{"textures":{}}"#, None),
        ("not json", None),
    ];
    for (json, model) in cases {
        let expected = model.map(|model| SkinTextures {
            url: url.into(),
            model,
        });
        assert_eq!(SkinTextures::of(&[textures(json)]), expected, "{json}");
    }
}
