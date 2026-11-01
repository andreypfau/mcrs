use std::process::Command;

/// A model or structure viewer bakes from the pack alone, without the network, a client or a
/// server.
#[test]
fn the_model_bake_links_no_network_client_or_server() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--offline",
            "--locked",
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
                "mcrs_minecraft_network"
                    | "mcrs_minecraft_protocol"
                    | "mcrs_minecraft_client"
                    | "mcrs_minecraft_client_world"
                    | "mcrs_minecraft_server"
                    | "mcrs_minecraft_engine_server"
                    | "tokio"
            )
        })
        .collect();
    assert!(forbidden.is_empty(), "the model bake links {forbidden:?}");
}
