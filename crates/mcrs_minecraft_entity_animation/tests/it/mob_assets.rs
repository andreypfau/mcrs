//! Writes the keyframe clips of the frog from the clip artefact, and checks that the file in the
//! repository is current.

use std::fmt::Write;

use mcrs_minecraft_fixtures::fixture;

use crate::artefact::{ChannelRecord, ClipArtefact, ClipRecord, KeyframeRecord};
use crate::pack;

const FROG_ANIMATIONS: &str = "animations/frog.animation.json";

/// The settings of a clip that the artefact does not give.
struct Settings {
    id: &'static str,
    /// The loop mode of a clip that the artefact does not mark as looping.
    hold: bool,
    anim_time_update: Option<&'static str>,
}

const FROG: [Settings; 6] = [
    Settings {
        id: "frog/jump",
        hold: true,
        anim_time_update: None,
    },
    Settings {
        id: "frog/croak",
        hold: true,
        anim_time_update: None,
    },
    Settings {
        id: "frog/tongue",
        hold: true,
        anim_time_update: None,
    },
    Settings {
        id: "frog/walk",
        hold: false,
        anim_time_update: Some("q.modified_distance_moved * 0.075"),
    },
    Settings {
        id: "frog/swim",
        hold: false,
        anim_time_update: Some("q.modified_distance_moved * 0.05"),
    },
    Settings {
        id: "frog/idle_water",
        hold: false,
        anim_time_update: None,
    },
];

/// The head scale of the tongue clip differs from the reference and is dropped.
fn dropped(clip: &str, bone: &str, channel: &str) -> bool {
    clip == "frog/tongue" && bone == "head" && channel == "scale"
}

fn number(value: f64) -> String {
    let rounded = (value * 1e5).round() / 1e5 + 0.0;
    serde_json::to_string(&rounded).unwrap()
}

fn vector(value: [f64; 3]) -> String {
    format!(
        "[{}, {}, {}]",
        number(value[0]),
        number(value[1]),
        number(value[2])
    )
}

/// A value of the artefact in file space.
fn file_value(channel: &str, [x, y, z]: [f64; 3]) -> [f64; 3] {
    match channel {
        "position" => [x, -y, z],
        "rotation" => [x.to_degrees(), y.to_degrees(), z.to_degrees()],
        "scale" => [x + 1.0, y + 1.0, z + 1.0],
        other => panic!("a channel of the artefact has the target {other}"),
    }
}

fn keyframe(channel: &str, key: &KeyframeRecord) -> String {
    let pre = file_value(channel, key.pre);
    let post = file_value(channel, key.post);
    let lerp_mode = match key.interpolation.as_str() {
        "linear" => "linear",
        "catmullrom" => "catmullrom",
        other => panic!("a keyframe of the artefact has the interpolation {other}"),
    };
    let time = number(key.time);
    match (vector(pre) == vector(post), lerp_mode) {
        (true, "linear") => format!("\"{time}\": {}", vector(post)),
        (true, _) => format!(
            "\"{time}\": {{ \"post\": {}, \"lerp_mode\": \"{lerp_mode}\" }}",
            vector(post)
        ),
        (false, _) => format!(
            "\"{time}\": {{ \"pre\": {}, \"post\": {}, \"lerp_mode\": \"{lerp_mode}\" }}",
            vector(pre),
            vector(post)
        ),
    }
}

fn channel(out: &mut String, record: &ChannelRecord) {
    let keys: Vec<String> = record
        .keyframes
        .iter()
        .map(|key| format!("            {}", keyframe(&record.target, key)))
        .collect();
    write!(
        out,
        "          \"{}\": {{\n{}\n          }}",
        record.target,
        keys.join(",\n")
    )
    .unwrap();
}

fn clip(out: &mut String, record: &ClipRecord, settings: &Settings) {
    let (_, name) = record.id.split_once('/').unwrap();
    let owner = record.id.split('/').next().unwrap();
    let loop_mode = match (record.looping, settings.hold) {
        (true, _) => "true",
        (false, true) => "\"hold_on_last_frame\"",
        (false, false) => "false",
    };
    writeln!(out, "    \"animation.{owner}.{name}\": {{").unwrap();
    writeln!(out, "      \"loop\": {loop_mode},").unwrap();
    writeln!(
        out,
        "      \"animation_length\": {},",
        number(record.length)
    )
    .unwrap();
    if let Some(update) = settings.anim_time_update {
        writeln!(out, "      \"anim_time_update\": \"{update}\",").unwrap();
    }
    writeln!(out, "      \"bones\": {{").unwrap();
    let bones: Vec<String> = record
        .bones
        .iter()
        .map(|(bone, channels)| {
            let mut text = format!("        \"{bone}\": {{\n");
            let kept: Vec<&ChannelRecord> = channels
                .iter()
                .filter(|channel| !dropped(&record.id, bone, &channel.target))
                .collect();
            for (index, record) in kept.iter().enumerate() {
                channel(&mut text, record);
                text.push_str(if index + 1 < kept.len() { ",\n" } else { "\n" });
            }
            text.push_str("        }");
            text
        })
        .collect();
    writeln!(out, "{}", bones.join(",\n")).unwrap();
    writeln!(out, "      }}").unwrap();
    out.push_str("    }");
}

fn generate() -> String {
    let artefact = ClipArtefact::parse(&fixture("entity_animations").text());
    let mut out = String::from("{\n  \"format_version\": \"1.8.0\",\n  \"animations\": {\n");
    for (index, settings) in FROG.iter().enumerate() {
        clip(&mut out, artefact.clip(settings.id), settings);
        out.push_str(if index + 1 < FROG.len() { ",\n" } else { "\n" });
    }
    out.push_str("  }\n}\n");
    out
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "a developer asks for the files to be written again through the environment"
)]
fn the_frog_clips_are_generated_from_the_clip_artefact() {
    let generated = generate();
    let path = pack::entity_pack().join(FROG_ANIMATIONS);
    if std::env::var("MCRS_BLESS").is_ok() {
        std::fs::write(&path, &generated).unwrap();
        return;
    }
    let held = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        held == generated,
        "assets/mcrs/entity/{FROG_ANIMATIONS} is stale; rerun with MCRS_BLESS=1 to write it again"
    );
}
