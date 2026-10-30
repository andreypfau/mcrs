use crate::metadata::{Kind, Workspace};
use crate::scope::{self, Head, Scope};
use crate::trigger::{Check, Trigger};
use crate::{keys, parity};
use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub type Outcome = Result<String, String>;

fn is_actions(value: Option<&str>) -> bool {
    value == Some("true")
}

const WASM32_PACKAGES: [&str; 10] = [
    "mcrs_minecraft_protocol",
    "mcrs_minecraft_wire",
    "mcrs_minecraft_keys",
    "mcrs_minecraft_anvil",
    "mcrs_minecraft_registry_catalog",
    "mcrs_minecraft_game_rule",
    "mcrs_minecraft_predicate",
    "mcrs_minecraft_enchantment",
    "mcrs_minecraft_loot",
    "mcrs_minecraft_biome_file",
];

const NO_STD_PACKAGES: [&str; 1] = ["mcrs_command"];

const NO_BEVY_PACKAGES: [&str; 61] = [
    "mcrs_command",
    "mcrs_minecraft_anvil",
    "mcrs_minecraft_biome",
    "mcrs_minecraft_biome_file",
    "mcrs_minecraft_block",
    "mcrs_minecraft_block_entity",
    "mcrs_minecraft_block_predicate",
    "mcrs_minecraft_chat",
    "mcrs_minecraft_chunk",
    "mcrs_minecraft_client_jar",
    "mcrs_minecraft_command",
    "mcrs_minecraft_command_argument",
    "mcrs_minecraft_command_argument_block",
    "mcrs_minecraft_command_argument_entity",
    "mcrs_minecraft_command_argument_vanilla",
    "mcrs_minecraft_command_registry",
    "mcrs_minecraft_command_tree",
    "mcrs_minecraft_core",
    "mcrs_minecraft_dimension",
    "mcrs_minecraft_dimension_environment",
    "mcrs_minecraft_enchantment",
    "mcrs_minecraft_entity",
    "mcrs_minecraft_environment",
    "mcrs_minecraft_fixtures",
    "mcrs_minecraft_game_mode",
    "mcrs_minecraft_game_rule",
    "mcrs_minecraft_heightmap",
    "mcrs_minecraft_item",
    "mcrs_minecraft_keys",
    "mcrs_minecraft_light",
    "mcrs_minecraft_light_color",
    "mcrs_minecraft_loot",
    "mcrs_minecraft_nbt",
    "mcrs_minecraft_network",
    "mcrs_minecraft_noise",
    "mcrs_minecraft_particle",
    "mcrs_minecraft_predicate",
    "mcrs_minecraft_profile",
    "mcrs_minecraft_protocol",
    "mcrs_minecraft_random",
    "mcrs_minecraft_registries",
    "mcrs_minecraft_registry_catalog",
    "mcrs_minecraft_simulation",
    "mcrs_minecraft_snbt",
    "mcrs_minecraft_sound",
    "mcrs_minecraft_text",
    "mcrs_minecraft_value_provider",
    "mcrs_minecraft_volume",
    "mcrs_minecraft_wire",
    "mcrs_minecraft_wire_macros",
    "mcrs_minecraft_worldgen",
    "mcrs_minecraft_worldgen_builtin",
    "mcrs_minecraft_worldgen_carver",
    "mcrs_minecraft_worldgen_density",
    "mcrs_minecraft_worldgen_feature",
    "mcrs_minecraft_worldgen_feature_place",
    "mcrs_minecraft_worldgen_generator",
    "mcrs_minecraft_worldgen_structure",
    "mcrs_minecraft_worldgen_structure_place",
    "mcrs_minecraft_worldgen_surface",
    "mcrs_minecraft_worldgen_testing",
];

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
    actions: bool,
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
            actions: is_actions(std::env::var("GITHUB_ACTIONS").ok().as_deref()),
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
        Check::Parity => parity_check(ctx),
        Check::Fmt => fmt(ctx),
        Check::Keys => keys::check(
            &ctx.root,
            &ctx.head,
            &scope::changed_paths(&ctx.root, ctx.base()?, &ctx.head)?,
            || ctx.exec("cargo", &keys::GENERATE),
        ),
        Check::Clippy => clippy(ctx),
        Check::Test => test(ctx),
        Check::NoBevy => no_bevy(ctx),
        Check::NoStd => build_for(
            ctx,
            "wasm32v1-none",
            &NO_STD_PACKAGES,
            "a crate that must build without std does not build for wasm32v1-none",
            "keep it #![no_std] and its dependencies free of std",
        ),
        Check::Wasm32 => build_for(
            ctx,
            "wasm32-unknown-unknown",
            &WASM32_PACKAGES,
            "a crate that must run in a browser does not build for wasm32-unknown-unknown",
            "keep its dependencies free of native-only code",
        ),
    }
}

fn build_for(
    ctx: &Context,
    target: &str,
    packages: &[&str],
    condition: &str,
    fix: &str,
) -> Outcome {
    let mut args = vec!["build", "--locked", "--target", target];
    for package in packages {
        args.extend(["-p", package]);
    }
    verdict(ctx.exec("cargo", &args)?, condition, fix)
}

fn no_bevy(ctx: &Context) -> Outcome {
    let mut args = vec!["check", "--locked", "--no-default-features"];
    for package in NO_BEVY_PACKAGES {
        args.extend(["-p", package]);
    }
    verdict(
        ctx.exec("cargo", &args)?,
        "a crate below layer 4 does not build without its default features",
        "put the code that needs Bevy behind the bevy feature",
    )
}

fn parity_check(ctx: &Context) -> Outcome {
    let base = ctx.base()?;
    let changed = scope::changed_paths(&ctx.root, base, &ctx.head)?;
    let selection = if ctx.trigger == Trigger::Commit {
        parity::Selection::Changed(&changed)
    } else {
        parity::Selection::Every
    };
    let annotate = ctx.actions.then_some(changed.as_slice());
    parity::check(&ctx.root, base, &ctx.head, selection, annotate)
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
    let mut args = vec!["clippy", "--workspace"];
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
    let passed = ctx.exec("cargo", &args)?;
    let core_selected = match &selection {
        Scope::Workspace => true,
        Scope::Packages(names) => names.iter().any(|n| n == "mcrs_minecraft_core"),
    };
    if !passed || ctx.trigger == Trigger::Gpu || !core_selected {
        return verdict(
            passed,
            "a test failed, or the selection ran no test",
            "fix the failing tests above; a crate with no default-tier test needs one",
        );
    }
    verdict(
        ctx.exec(
            "cargo",
            &["test", "--doc", "--locked", "-p", "mcrs_minecraft_core"],
        )?,
        "a core doctest failed",
        "fix the doctests above",
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
    fn only_the_value_true_means_a_run_under_actions() {
        let rows = [
            (Some("true"), true),
            (Some("false"), false),
            (Some("True"), false),
            (Some(""), false),
            (None, false),
        ];
        for (value, expected) in rows {
            assert_eq!(is_actions(value), expected, "{value:?}");
        }
    }

    #[test]
    fn parity_judges_the_changed_files_locally_and_every_file_elsewhere() {
        let claim = "#[test]\nfn vanilla_value() {\n    assert_eq!(got(), 1);\n}\n";
        let repo = Repo::new("parity-selection");
        repo.init();
        repo.write("crates/x/src/old.rs", claim);
        repo.commit("base");
        repo.git(&["branch", "base"]);
        repo.write("crates/x/src/new.rs", claim);
        let detail = |trigger| run(&context(&repo, trigger), Check::Parity);
        let one = Ok("1 files; 1 vanilla claims outside parity files".to_owned());
        let both = Ok("2 files; 2 vanilla claims outside parity files".to_owned());
        assert_eq!(detail(Trigger::Commit), one);
        assert_eq!(detail(Trigger::Lint), both);
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
