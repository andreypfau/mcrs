use std::collections::BTreeMap;

use mcrs_minecraft_client::options::ClientOptions;

/// The client options that give the options of a scene, which are keys of `options.txt` with
/// values as that file writes them. A feature that mcrs does not draw yet counts as off, so the
/// scene must ask for it off.
pub fn client_options(options: &BTreeMap<String, String>) -> Result<ClientOptions, String> {
    let mut client = ClientOptions::default();
    for (key, value) in options {
        match key.as_str() {
            "fov" => client.fov = (70.0 + number(key, value)? * 40.0).round() as u8,
            "fovEffectScale" => client.fov_effect_scale = number(key, value)?,
            "ao" => client.smooth_light = switch(key, value)?,
            "renderClouds" => client.clouds = clouds(value)?,
            "renderDistance" => {
                client.view_distance = Some(
                    value
                        .parse()
                        .map_err(|_| format!("{key} is {value}, not a render distance"))?,
                );
            }
            "gamma" => client.brightness = number(key, value)?,
            // mcrs has no view bobbing, entity shadow, vignette or improved transparency, no
            // entity distance cull and no section fade, and its block atlas always holds the
            // full mip chain of a 16-pixel sprite: four levels below the base.
            "bobView" | "entityShadows" | "vignette" | "improvedTransparency" => {
                absent(key, value, "false")?;
            }
            "entityDistanceScaling" => absent_number(key, value, 1.0)?,
            "chunkSectionFadeInTime" => absent_number(key, value, 0.0)?,
            "mipmapLevels" => absent_number(key, value, 4.0)?,
            _ => {
                return Err(format!(
                    "the scene sets {key}, which the harness does not know"
                ));
            }
        }
    }
    client.validate()?;
    Ok(client)
}

fn number(key: &str, value: &str) -> Result<f32, String> {
    value
        .parse()
        .map_err(|_| format!("{key} is {value}, not a number"))
}

fn switch(key: &str, value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("{key} is {value}, not true or false")),
    }
}

fn clouds(value: &str) -> Result<bool, String> {
    match value {
        "\"false\"" => Ok(false),
        "\"fast\"" | "\"true\"" => Ok(true),
        _ => Err(format!(
            "renderClouds is {value}, not \"false\", \"fast\" or \"true\""
        )),
    }
}

/// mcrs has no setting for this option, and what it draws matches `expected`.
fn absent(key: &str, value: &str, expected: &str) -> Result<(), String> {
    if value == expected {
        Ok(())
    } else {
        Err(format!(
            "{key} is {value}; mcrs only draws it as {expected}"
        ))
    }
}

fn absent_number(key: &str, value: &str, expected: f32) -> Result<(), String> {
    if number(key, value)? == expected {
        Ok(())
    } else {
        Err(format!(
            "{key} is {value}; mcrs only draws it as {expected}"
        ))
    }
}
