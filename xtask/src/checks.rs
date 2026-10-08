use crate::metadata::{Kind, Workspace};
use crate::scope::{self, Mode, Scope};
use crate::trigger::{Check, Trigger};
use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub type Outcome = Result<(), String>;

struct Changes {
    base_ref: String,
    paths: Vec<String>,
    scope: Scope,
}

pub struct Context {
    trigger: Trigger,
    root: PathBuf,
    base_ref: String,
    workspace: OnceCell<Result<Workspace, String>>,
    changes: OnceCell<Result<Changes, String>>,
}

impl Context {
    pub fn new(trigger: Trigger, root: PathBuf, base_ref: String) -> Context {
        Context {
            trigger,
            root,
            base_ref,
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

    fn changes(&self) -> Result<&Changes, String> {
        self.changes
            .get_or_init(|| self.compute_changes())
            .as_ref()
            .map_err(Clone::clone)
    }

    fn compute_changes(&self) -> Result<Changes, String> {
        let workspace = self.workspace()?;
        let base = scope::resolve_base(&self.root, &self.base_ref).map_err(|e| e.to_string())?;
        let mode = match self.trigger {
            Trigger::Edit | Trigger::Commit => Mode::Working,
            _ => Mode::Committed,
        };
        let paths = scope::changed_paths(&self.root, &base, mode)?;
        let bump = scope::touches_root_manifests(&paths)
            .then(|| scope::manifest_texts(&self.root, &base, mode))
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
        Ok(())
    } else {
        Err(format!("{condition}; {fix}"))
    }
}

pub fn run(ctx: &Context, check: Check) -> Outcome {
    match check {
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
            let args = [
                "build",
                "--locked",
                "--target",
                "wasm32-unknown-unknown",
                "-p",
                "mcrs_minecraft_protocol",
            ];
            verdict(
                ctx.exec("cargo", &args)?,
                "the protocol crate does not build for wasm32-unknown-unknown",
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
    let changes = ctx.changes()?;
    let workspace = ctx.workspace()?;
    let mut by_edition: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for path in changes
        .paths
        .iter()
        .filter(|p| p.ends_with(".rs") && ctx.root.join(p).exists())
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
        println!("no Rust file changed since {}", changes.base_ref);
        return Ok(());
    }
    let mut passed = true;
    for (edition, files) in by_edition {
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

fn clippy(ctx: &Context) -> Outcome {
    let mut args = vec!["clippy"];
    if ctx.trigger == Trigger::Commit {
        let changes = ctx.changes()?;
        match &changes.scope {
            Scope::Workspace => args.push("--workspace"),
            Scope::Packages(names) if names.is_empty() => {
                println!("no crate changed since {}", changes.base_ref);
                return Ok(());
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
                return Ok(());
            }
            ("ci", selection)
        }
    };
    let mut args = vec!["nextest", "run", "--profile", profile, "--locked"];
    match &selection {
        Scope::Workspace => args.push("--workspace"),
        Scope::Packages(names) => {
            for name in names {
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

pub fn root_of(start: &Path) -> Result<PathBuf, String> {
    let top = crate::git::text(start, &["rev-parse", "--show-toplevel"])
        .map_err(|e| format!("{e}; run this from inside the repository"))?;
    Ok(PathBuf::from(top.trim()))
}
