use crate::metadata::Workspace;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn check(root: &Path) -> Result<String, String> {
    let workspace = Workspace::load(root)?;
    let policy_path = root.join("examples/dependencies.json");
    let policy: BTreeMap<String, Vec<String>> = serde_json::from_slice(
        &fs::read(&policy_path).map_err(|error| format!("{}: {error}", policy_path.display()))?,
    )
    .map_err(|error| format!("{}: {error}", policy_path.display()))?;
    let examples: Vec<_> = workspace
        .packages
        .iter()
        .filter(|package| package.dir.replace('\\', "/").starts_with("examples/"))
        .collect();
    for name in policy.keys() {
        if !examples.iter().any(|package| &package.name == name) {
            return Err(format!("dependency policy names missing example {name}"));
        }
    }
    for package in &examples {
        let forbidden = policy.get(&package.name).ok_or_else(|| {
            format!(
                "example {} needs an entry in examples/dependencies.json",
                package.name
            )
        })?;
        let output = Command::new("cargo")
            .args([
                "tree",
                "--locked",
                "--config",
                "resolver.feature-unification=\"selected\"",
                "--package",
                &package.name,
                "--edges",
                "normal",
                "--prefix",
                "depth",
                "--format",
                "{p}",
                "--color",
                "never",
                "--target",
                "all",
            ])
            .current_dir(root)
            .output()
            .map_err(|error| format!("cargo tree for {}: {error}", package.name))?;
        if !output.status.success() {
            return Err(format!(
                "cargo tree for {}: {}",
                package.name,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        check_tree(
            &package.name,
            forbidden,
            &String::from_utf8_lossy(&output.stdout),
        )?;
    }
    Ok(format!(
        "{} example dependency policies checked",
        examples.len()
    ))
}

fn check_tree(example: &str, forbidden: &[String], tree: &str) -> Result<(), String> {
    let mut path = Vec::new();
    for line in tree.lines().filter(|line| !line.is_empty()) {
        let depth_end = line
            .find(|character: char| !character.is_ascii_digit())
            .ok_or_else(|| format!("invalid dependency tree line: {line}"))?;
        let depth: usize = line[..depth_end]
            .parse()
            .map_err(|error| format!("invalid dependency depth: {error}"))?;
        let name = line[depth_end..]
            .split_whitespace()
            .next()
            .ok_or_else(|| format!("missing dependency name: {line}"))?;
        if depth > path.len() {
            return Err(format!("missing dependency parent: {line}"));
        }
        path.truncate(depth);
        path.push(name);
        if depth > 0
            && forbidden
                .iter()
                .any(|pattern| match pattern.strip_suffix('*') {
                    Some(prefix) => name.starts_with(prefix),
                    None => name == pattern,
                })
        {
            return Err(format!(
                "example {example} has a forbidden dependency: {}",
                path.join(" -> ")
            ));
        }
    }
    if path.is_empty() {
        return Err(format!(
            "cargo returned an empty dependency tree for {example}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_the_branch_that_reaches_a_forbidden_dependency() {
        let forbidden = vec!["bevy*".to_owned()];
        for (tree, expected) in [
            (
                "0ping v0.1.0\n1network v0.1.0\n2bevy_ecs v0.1.0\n",
                Some("ping -> network -> bevy_ecs"),
            ),
            (
                "0ping v0.1.0\n1allowed v0.1.0\n2leaf v0.1.0\n1bevy v0.1.0\n",
                Some("ping -> bevy"),
            ),
            ("0ping v0.1.0\n1network v0.1.0\n2tokio v0.1.0\n", None),
            (
                "0ping v0.1.0\n1network v0.1.0\n2tokio v0.1.0\n1tokio v0.1.0 (*)\n1bevy v0.1.0\n",
                Some("ping -> bevy"),
            ),
        ] {
            let result = check_tree("ping", &forbidden, tree);
            match expected {
                Some(path) => assert!(
                    result.unwrap_err().contains(path),
                    "the diagnostic must show the dependency path"
                ),
                None => assert!(result.is_ok(), "allowed dependencies must pass: {result:?}"),
            }
        }
    }

    #[test]
    fn workspace_examples_obey_dependency_policies() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        check(root).unwrap();
    }
}

#[cfg(test)]
#[test]
fn resolves_normal_dependencies_without_test_features() {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Fixture(PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    let root = std::env::temp_dir().join(format!(
        "mcrs-example-dependencies-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let fixture = Fixture(root);
    {
        let root = &fixture.0;
        fs::create_dir_all(root.join("examples/probe/src")).unwrap();
        fs::create_dir_all(root.join("bridge/src")).unwrap();
        fs::create_dir_all(root.join("forbidden/src")).unwrap();
        fs::create_dir_all(root.join(".cargo")).unwrap();
        fs::write(
            root.join(".cargo/config.toml"),
            "[unstable]\nfeature-unification = true\n",
        )
        .unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = [\"examples/probe\", \"bridge\", \"forbidden\"]\nresolver = \"2\"\n").unwrap();
        fs::write(root.join("bridge/Cargo.toml"), "[package]\nname = \"bridge\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\nforbidden = { path = \"../forbidden\", optional = true }\n").unwrap();
        fs::write(
            root.join("forbidden/Cargo.toml"),
            "[package]\nname = \"forbidden\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        for path in [
            "examples/probe/src/main.rs",
            "bridge/src/lib.rs",
            "forbidden/src/lib.rs",
        ] {
            fs::write(root.join(path), "fn main() {}\n").unwrap();
        }
        fs::write(
            root.join("examples/dependencies.json"),
            "{\"probe\":[\"forbidden\"]}",
        )
        .unwrap();
    }
    for (dependencies, expected) in [
        (
            "[dependencies]\nbridge = { path = \"../../bridge\" }\n[dev-dependencies]\nbridge = { path = \"../../bridge\", features = [\"forbidden\"] }\n",
            None,
        ),
        (
            "[dependencies]\nbridge = { path = \"../../bridge\", features = [\"forbidden\"] }\n",
            Some("probe -> bridge -> forbidden"),
        ),
        (
            "[dependencies]\nalias = { package = \"forbidden\", path = \"../../forbidden\" }\n",
            Some("probe -> forbidden"),
        ),
    ] {
        let manifest = format!(
            "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n{dependencies}"
        );
        fs::write(fixture.0.join("examples/probe/Cargo.toml"), manifest).unwrap();
        let status = Command::new("cargo")
            .args(["generate-lockfile", "--offline"])
            .current_dir(&fixture.0)
            .status()
            .unwrap();
        assert!(
            status.success(),
            "the local fixture must resolve without a registry"
        );
        let result = check(&fixture.0);
        match expected {
            Some(path) => assert!(
                result.unwrap_err().contains(path),
                "the forbidden dependency must include its path"
            ),
            None => assert!(
                result.is_ok(),
                "development features must not leak into the executable: {result:?}"
            ),
        }
    }
    let path = fixture.0.clone();
    drop(fixture);
    assert!(
        !path.exists(),
        "the fixture must remove its temporary workspace"
    );
}
