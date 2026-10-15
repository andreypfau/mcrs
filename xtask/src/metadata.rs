use serde::Deserialize;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Normal,
    Build,
    Dev,
}

#[derive(Debug)]
pub struct Dependency {
    pub name: String,
    pub kind: Kind,
    pub local: bool,
}

#[derive(Debug)]
pub struct Package {
    pub name: String,
    pub dir: String,
    pub edition: String,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug)]
pub struct Workspace {
    pub packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Metadata {
    workspace_root: String,
    packages: Vec<MetadataPackage>,
}

#[derive(Deserialize)]
struct MetadataPackage {
    name: String,
    manifest_path: String,
    edition: String,
    dependencies: Vec<MetadataDependency>,
}

#[derive(Deserialize)]
struct MetadataDependency {
    name: String,
    kind: Option<Kind>,
    path: Option<String>,
}

const ROOT_OWNED: [&str; 4] = ["src/", "tests/", "benches/", "examples/"];

impl Workspace {
    pub fn load(repo: &Path) -> Result<Workspace, String> {
        let output = Command::new("cargo")
            .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
            .current_dir(repo)
            .output()
            .map_err(|e| {
                format!("could not run cargo metadata: {e}; install the Rust toolchain")
            })?;
        if !output.status.success() {
            return Err(format!(
                "cargo metadata failed: {}; fix the manifests or commit the updated Cargo.lock",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Workspace::parse(&String::from_utf8_lossy(&output.stdout))
    }

    pub fn parse(json: &str) -> Result<Workspace, String> {
        let metadata: Metadata =
            serde_json::from_str(json).map_err(|e| format!("cargo metadata output: {e}"))?;
        let root = metadata.workspace_root;
        let packages = metadata
            .packages
            .into_iter()
            .map(|package| {
                let dir = Path::new(&package.manifest_path)
                    .parent()
                    .and_then(|dir| dir.strip_prefix(&root).ok())
                    .and_then(Path::to_str)
                    .ok_or_else(|| {
                        format!(
                            "package {} lies outside the workspace root {root}",
                            package.name
                        )
                    })?
                    .to_owned();
                let dependencies = package
                    .dependencies
                    .into_iter()
                    .map(|dep| Dependency {
                        name: dep.name,
                        kind: dep.kind.unwrap_or(Kind::Normal),
                        local: dep.path.is_some(),
                    })
                    .collect();
                Ok(Package {
                    name: package.name,
                    dir,
                    edition: package.edition,
                    dependencies,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Workspace { packages })
    }

    pub fn root_package(&self) -> Option<&Package> {
        self.packages.iter().find(|p| p.dir.is_empty())
    }

    pub fn owner(&self, path: &str) -> Option<&Package> {
        let nested = self
            .packages
            .iter()
            .filter(|p| !p.dir.is_empty())
            .filter(|p| {
                path.strip_prefix(p.dir.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
            })
            .max_by_key(|p| p.dir.len());
        nested.or_else(|| {
            let owned = path == "build.rs" || ROOT_OWNED.iter().any(|dir| path.starts_with(dir));
            self.root_package().filter(|_| owned)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xtask_declares_no_dependency_on_a_workspace_crate() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let workspace = Workspace::load(repo).unwrap();
        let xtask = workspace
            .packages
            .iter()
            .find(|p| p.name == "xtask")
            .unwrap();
        let offending: Vec<_> = xtask
            .dependencies
            .iter()
            .filter(|dep| dep.name.starts_with("mcrs_"))
            .collect();
        assert!(offending.is_empty(), "{offending:?}");
        assert_eq!(xtask.dir, "xtask");
    }
}
