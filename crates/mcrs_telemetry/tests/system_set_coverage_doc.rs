use std::path::Path;

#[test]
fn readme_documents_per_system_span_strategy() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let readme_path = Path::new(manifest_dir).join("README.md");
    let readme = std::fs::read_to_string(&readme_path)
        .expect("crate-root README.md must exist");

    assert!(
        readme.contains("bevy_ecs/trace"),
        "crate README must cite the per-system span strategy (looked for 'bevy_ecs/trace' substring)"
    );

    assert!(
        readme.contains("Bevy 0.18") && readme.contains("no public API"),
        "crate README must describe the Bevy 0.18 wrapper-API constraint as engineering rationale \
         (looked for 'Bevy 0.18' and 'no public API' substrings)"
    );
}
