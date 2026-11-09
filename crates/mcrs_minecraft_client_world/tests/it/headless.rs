use std::process::Command;

/// A bot or a headless client built on the world state links no window, renderer or mesher.
#[test]
fn the_client_world_links_no_window_or_renderer() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--offline",
            "--locked",
            "--config",
            "resolver.feature-unification=\"selected\"",
            "--package",
            env!("CARGO_PKG_NAME"),
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8(output.stdout).unwrap();
    let forbidden: Vec<&str> = tree
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|&name| {
            matches!(
                name,
                "bevy_render"
                    | "bevy_winit"
                    | "bevy_window"
                    | "bevy_pbr"
                    | "mcrs_minecraft_light_render"
                    | "mcrs_minecraft_mesh"
            ) || name.starts_with("mcrs_minecraft_render")
        })
        .collect();
    assert!(
        forbidden.is_empty(),
        "the client world state links {forbidden:?}"
    );
}
