use std::collections::BTreeMap;

/// One `MCRS_` setting of the client process.
pub struct Knob {
    pub name: &'static str,
    pub value: String,
}

/// The client settings that give the options of a scene, which are keys of `options.txt` with
/// values as that file writes them. A feature that mcrs does not draw yet counts as off, so the
/// scene must ask for it off.
pub fn knobs(options: &BTreeMap<String, String>) -> Result<Vec<Knob>, String> {
    let mut knobs = Vec::new();
    for (key, value) in options {
        let knob = |name, value: String| Some(Knob { name, value });
        let made = match key.as_str() {
            "fov" => knob("FOV", fov_degrees(value)?.to_string()),
            "fovEffectScale" => knob("FOV_EFFECT", number(key, value)?.to_string()),
            "ao" => knob("SMOOTH_LIGHT", switch(key, value)?.to_owned()),
            "renderClouds" => knob("CLOUDS", clouds(value)?.to_owned()),
            "renderDistance" => knob("VIEW", value.clone()),
            "gamma" => knob("BRIGHTNESS", number(key, value)?.to_string()),
            // mcrs has no view bobbing, entity shadow, vignette or improved transparency, no
            // entity distance cull and no section fade, and its block atlas always holds the
            // full mip chain of a 16-pixel sprite: four levels below the base.
            "bobView" | "entityShadows" | "vignette" | "improvedTransparency" => {
                absent(key, value, "false")?
            }
            "entityDistanceScaling" => absent_number(key, value, 1.0)?,
            "chunkSectionFadeInTime" => absent_number(key, value, 0.0)?,
            "mipmapLevels" => absent_number(key, value, 4.0)?,
            _ => {
                return Err(format!(
                    "the scene sets {key}, which the harness does not know"
                ));
            }
        };
        knobs.extend(made);
    }
    Ok(knobs)
}

/// `options.txt` writes the field of view as `(degrees - 70) / 40`.
fn fov_degrees(value: &str) -> Result<u8, String> {
    let degrees = 70.0 + number("fov", value)? * 40.0;
    if (30.0..=110.0).contains(&degrees) {
        Ok(degrees.round() as u8)
    } else {
        Err(format!(
            "fov {value} is {degrees} degrees, out of 30 to 110"
        ))
    }
}

fn number(key: &str, value: &str) -> Result<f32, String> {
    value
        .parse()
        .map_err(|_| format!("{key} is {value}, not a number"))
}

fn switch(key: &str, value: &str) -> Result<&'static str, String> {
    match value {
        "true" => Ok("1"),
        "false" => Ok("0"),
        _ => Err(format!("{key} is {value}, not true or false")),
    }
}

fn clouds(value: &str) -> Result<&'static str, String> {
    match value {
        "\"false\"" => Ok("0"),
        "\"fast\"" | "\"true\"" => Ok("1"),
        _ => Err(format!(
            "renderClouds is {value}, not \"false\", \"fast\" or \"true\""
        )),
    }
}

/// mcrs has no setting for this option, and what it draws matches `expected`.
fn absent(key: &str, value: &str, expected: &str) -> Result<Option<Knob>, String> {
    if value == expected {
        Ok(None)
    } else {
        Err(format!(
            "{key} is {value}; mcrs only draws it as {expected}"
        ))
    }
}

fn absent_number(key: &str, value: &str, expected: f32) -> Result<Option<Knob>, String> {
    if number(key, value)? == expected {
        Ok(None)
    } else {
        Err(format!(
            "{key} is {value}; mcrs only draws it as {expected}"
        ))
    }
}
