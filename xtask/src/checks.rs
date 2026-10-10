use crate::metadata::{Kind, Workspace};
use crate::scope::{self, Head, Scope};
use crate::trigger::{Check, Trigger};
use crate::{gate, marker, size, unused_deps};
use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub type Outcome = Result<String, String>;

const WASM32_PACKAGES: [&str; 9] = [
    "mcrs_minecraft_protocol",
    "mcrs_minecraft_keys",
    "mcrs_minecraft_anvil",
    "mcrs_minecraft_registry_catalog",
    "mcrs_minecraft_game_rule",
    "mcrs_minecraft_predicate",
    "mcrs_minecraft_enchantment",
    "mcrs_minecraft_loot",
    "mcrs_minecraft_biome_file",
];

pub(crate) fn capture(
    root: &Path,
    program: &str,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<std::process::Output, String> {
    Command::new(program)
        .args(args)
        .envs(env.iter().copied())
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run {program}: {e}; install it and put it on PATH"))
}

struct Changes {
    base_ref: String,
    paths: Vec<String>,
    scope: Scope,
}

pub struct Context {
    trigger: Trigger,
    root: PathBuf,
    base_ref: String,
    head: Head,
    base: OnceCell<Result<String, String>>,
    workspace: OnceCell<Result<Workspace, String>>,
    changes: OnceCell<Result<Changes, String>>,
}

impl Context {
    pub fn new(trigger: Trigger, root: PathBuf, base_ref: String, head: Head) -> Context {
        Context {
            trigger,
            root,
            base_ref,
            head,
            base: OnceCell::new(),
            workspace: OnceCell::new(),
            changes: OnceCell::new(),
        }
    }

    fn workspace(&self) -> Result<&Workspace, String> {
        self.workspace
            .get_or_init(|| Workspace::load(&self.root))
            .as_ref()
            .map_err(Clone::clone)
    }

    fn base(&self) -> Result<&str, String> {
        self.base
            .get_or_init(|| {
                scope::resolve_base(&self.root, &self.base_ref, &self.head)
                    .map_err(|e| e.to_string())
            })
            .as_ref()
            .map(String::as_str)
            .map_err(Clone::clone)
    }

    fn changes(&self) -> Result<&Changes, String> {
        self.changes
            .get_or_init(|| self.compute_changes())
            .as_ref()
            .map_err(Clone::clone)
    }

    fn compute_changes(&self) -> Result<Changes, String> {
        let workspace = self.workspace()?;
        let base = self.base()?;
        let paths = scope::changed_paths(&self.root, base, &self.head)?;
        let bump = scope::touches_root_manifests(&paths)
            .then(|| scope::manifest_texts(&self.root, base, &self.head))
            .flatten()
            .and_then(|texts| scope::version_bump(workspace, &texts));
        if let Some(bump) = &bump {
            println!(
                "Cargo.toml and Cargo.lock: workspace version {} to {} only",
                bump.from, bump.to
            );
        }
        let scope = scope::scope(workspace, &paths, bump.is_some());
        Ok(Changes {
            base_ref: self.base_ref.clone(),
            paths,
            scope,
        })
    }

    fn exec(&self, program: &str, args: &[&str]) -> Result<bool, String> {
        let shown = format!("{program} {}", args.join(" "));
        println!("$ {}", shown.trim_end());
        let resolved = if program.contains('/') {
            self.root.join(program)
        } else {
            PathBuf::from(program)
        };
        Command::new(&resolved)
            .args(args)
            .current_dir(&self.root)
            .status()
            .map(|status| status.success())
            .map_err(|e| format!("could not run {program}: {e}; install it and put it on PATH"))
    }
}

fn verdict(passed: bool, condition: &str, fix: &str) -> Outcome {
    if passed {
        Ok(String::new())
    } else {
        Err(format!("{condition}; {fix}"))
    }
}

pub fn run(ctx: &Context, check: Check) -> Outcome {
    match check {
        Check::Size => size::check(&ctx.root, ctx.base()?, &ctx.head),
        Check::GateConfig => gate::check(&ctx.root, ctx.base()?, &ctx.head),
        Check::MutationMarker => marker::check(&ctx.root, &ctx.head),
        Check::UnusedDeps => {
            unused_deps::check(&ctx.root, ctx.base()?, &ctx.head, ctx.workspace()?)
        }
        Check::Fmt => fmt(ctx),
        Check::Clippy => clippy(ctx),
        Check::Test => test(ctx),
        Check::NoBevy => {
            let passed = ctx.exec("scripts/check-no-bevy.sh", &[])?;
            verdict(
                passed,
                "a crate that must build without Bevy pulls it in or fails to build",
                "remove the dependency or put it behind the bevy feature",
            )
        }
        Check::Wasm32 => {
            let mut args = vec!["build", "--locked", "--target", "wasm32-unknown-unknown"];
            for package in WASM32_PACKAGES {
                args.extend(["-p", package]);
            }
            verdict(
                ctx.exec("cargo", &args)?,
                "a crate that must run in a browser does not build for wasm32-unknown-unknown",
                "keep its dependencies free of native-only code",
            )
        }
        Check::ExportedBuild => verdict(
            ctx.exec("scripts/check-exported-build.sh", &[])?,
            "a tree exported from git does not build or its identity tests fail",
            "commit every file the build needs",
        ),
    }
}

fn fmt(ctx: &Context) -> Outcome {
    const CONDITION: &str = "files need formatting";
    const FIX: &str = "run cargo fmt --all";
    if !matches!(ctx.trigger, Trigger::Edit | Trigger::Commit) {
        return verdict(
            ctx.exec("cargo", &["fmt", "--all", "--check"])?,
            CONDITION,
            FIX,
        );
    }
    let workspace = ctx.workspace()?;
    let staged = ctx.trigger == Trigger::Commit;
    let (paths, base_ref) = if staged {
        (
            scope::staged_paths(&ctx.root, ctx.base()?)?,
            ctx.base_ref.as_str(),
        )
    } else {
        let changes = ctx.changes()?;
        (changes.paths.clone(), changes.base_ref.as_str())
    };
    let mut by_edition: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for path in paths
        .iter()
        .filter(|p| p.ends_with(".rs") && (staged || ctx.root.join(p).exists()))
    {
        let owner = workspace
            .owner(path)
            .or_else(|| workspace.root_package())
            .ok_or_else(|| format!("no package owns {path}; add it to a workspace crate"))?;
        by_edition
            .entry(owner.edition.as_str())
            .or_default()
            .push(path);
    }
    if by_edition.is_empty() {
        println!("no Rust file changed since {base_ref}");
        return Ok(String::new());
    }
    let mut passed = true;
    for (edition, files) in by_edition {
        if staged {
            for file in files {
                passed &= rustfmt_index(ctx, edition, file)?;
            }
            continue;
        }
        let mut args = vec![
            "--check",
            "--edition",
            edition,
            "--config",
            "skip_children=true",
        ];
        args.extend(files);
        passed &= ctx.exec("rustfmt", &args)?;
    }
    verdict(passed, CONDITION, FIX)
}

fn rustfmt_index(ctx: &Context, edition: &str, path: &str) -> Result<bool, String> {
    let blob =
        crate::git::run(&ctx.root, &["show", &format!(":{path}")]).map_err(|e| e.to_string())?;
    println!("$ rustfmt --edition {edition} < :{path}");
    let mut child = Command::new("rustfmt")
        .args(["--edition", edition, "--config", "skip_children=true"])
        .current_dir(&ctx.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run rustfmt: {e}; install it and put it on PATH"))?;
    let mut stdin = child.stdin.take().ok_or("rustfmt has no standard input")?;
    stdin
        .write_all(&blob)
        .map_err(|e| format!("could not pass {path} to rustfmt: {e}"))?;
    drop(stdin);
    let formatted = child
        .wait_with_output()
        .map_err(|e| format!("could not wait for rustfmt: {e}"))?;
    let clean = formatted.status.success() && formatted.stdout == blob;
    if !clean {
        println!("{path}: the staged contents are not formatted");
    }
    Ok(clean)
}

fn clippy(ctx: &Context) -> Outcome {
    let mut args = vec!["clippy"];
    if ctx.trigger == Trigger::Commit {
        let changes = ctx.changes()?;
        match &changes.scope {
            Scope::Workspace => args.push("--workspace"),
            Scope::Packages(names) if names.is_empty() => {
                println!("no crate changed since {}", changes.base_ref);
                return Ok(String::new());
            }
            Scope::Packages(names) => {
                for name in names {
                    args.extend(["-p", name.as_str()]);
                }
            }
        }
    } else {
        args.push("--workspace");
    }
    args.extend(["--all-targets", "--locked", "--", "-D", "warnings"]);
    verdict(
        ctx.exec("cargo", &args)?,
        "clippy reported diagnostics",
        "resolve the diagnostics above",
    )
}

fn test(ctx: &Context) -> Outcome {
    let (profile, selection) = match ctx.trigger {
        Trigger::Full => ("full", Scope::Workspace),
        Trigger::Gpu => ("gpu", Scope::Packages(gpu_packages(ctx.workspace()?)?)),
        _ => {
            let changes = ctx.changes()?;
            let selection = changes.scope.clone().with_dependents(ctx.workspace()?);
            if selection == Scope::Packages(Vec::new()) {
                println!(
                    "no crate changed since {}; nothing to test",
                    changes.base_ref
                );
                return Ok(String::new());
            }
            ("ci", selection)
        }
    };
    let local_only = if std::env::var_os("CI").is_some() && ctx.trigger != Trigger::Gpu {
        local_only_packages(ctx.workspace()?)
    } else {
        Vec::new()
    };
    let mut args = vec!["nextest", "run", "--profile", profile, "--locked"];
    match &selection {
        Scope::Workspace => {
            args.push("--workspace");
            for name in &local_only {
                args.extend(["--exclude", name.as_str()]);
            }
        }
        Scope::Packages(names) => {
            let kept: Vec<&String> = names.iter().filter(|n| !local_only.contains(n)).collect();
            if kept.is_empty() {
                println!("only crates that are tested locally changed; nothing to test here");
                return Ok(String::new());
            }
            for name in kept {
                args.extend(["-p", name.as_str()]);
            }
        }
    }
    verdict(
        ctx.exec("cargo", &args)?,
        "a test failed, or the selection ran no test",
        "fix the failing tests above; a crate with no default-tier test needs one",
    )
}

fn gpu_packages(workspace: &Workspace) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = workspace
        .packages
        .iter()
        .filter(|p| {
            p.dependencies
                .iter()
                .any(|d| d.name == "wgpu" && d.kind != Kind::Dev)
        })
        .map(|p| p.name.clone())
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Err(
            "no workspace crate depends on wgpu; add the crate that holds the GPU tests".to_owned(),
        );
    }
    Ok(names)
}

fn local_only_packages(workspace: &Workspace) -> Vec<String> {
    let mut names: Vec<String> = workspace
        .packages
        .iter()
        .filter(|p| p.dependencies.iter().any(|d| d.name == "wgpu"))
        .map(|p| p.name.clone())
        .collect();
    loop {
        let before = names.len();
        for p in &workspace.packages {
            if !names.contains(&p.name)
                && p.dependencies
                    .iter()
                    .any(|d| d.local && names.contains(&d.name))
            {
                names.push(p.name.clone());
            }
        }
        if names.len() == before {
            break;
        }
    }
    names.sort();
    names
}

pub fn root_of(start: &Path) -> Result<PathBuf, String> {
    let top = crate::git::text(start, &["rev-parse", "--show-toplevel"])
        .map_err(|e| format!("{e}; run this from inside the repository"))?;
    Ok(PathBuf::from(top.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Package;
    use crate::scope::tests::Repo;

    fn context(repo: &Repo, trigger: Trigger) -> Context {
        let ctx = Context::new(trigger, repo.0.clone(), "base".to_owned(), Head::Worktree);
        let workspace = Workspace {
            packages: vec![Package {
                name: "x".to_owned(),
                dir: "crates/x".to_owned(),
                edition: "2024".to_owned(),
                dependencies: Vec::new(),
            }],
        };
        ctx.workspace.set(Ok(workspace)).unwrap();
        ctx
    }

    #[test]
    fn the_commit_trigger_formats_what_is_staged_and_nothing_else() {
        let repo = Repo::new("fmt-staged");
        repo.init();
        repo.write("crates/x/src/lib.rs", "fn a() {}\n");
        repo.commit("base");
        repo.git(&["branch", "base"]);
        let formats = |trigger| run(&context(&repo, trigger), Check::Fmt).is_ok();
        let draft = "crates/x/src/draft.rs";

        repo.write(draft, "fn   b( ){ }\n");
        assert!(formats(Trigger::Commit), "an untracked file was checked");
        assert!(!formats(Trigger::Edit));

        repo.git(&["add", draft]);
        repo.write(draft, "fn b() {}\n");
        assert!(
            !formats(Trigger::Commit),
            "the worktree copy was checked instead of the staged one"
        );

        repo.git(&["add", draft]);
        repo.write(draft, "fn   b( ){ }\n");
        assert!(
            formats(Trigger::Commit),
            "an unstaged edit was checked instead of the staged copy"
        );
    }
}
