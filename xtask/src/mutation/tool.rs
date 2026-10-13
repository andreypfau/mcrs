use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Output};

pub(crate) const VERSION: &str = "27.1.0";
pub(crate) const TEST_TIMEOUT_SECONDS: u32 = 360;
const BUILD_TIMEOUT_SECONDS: u32 = 840;
pub(crate) const MATRIX_TEST_TIMEOUT_SECONDS: u32 = 360;
pub(crate) const MUTANTS_PER_SHARD: usize = 23;
pub(crate) const SWEEP_MUTANT_BUDGET: usize = 2760;

const SCRUBBED_PREFIXES: [&str; 2] = ["CARGO_MUTANTS_", "NEXTEST_"];

pub(crate) fn version_matches(output: &str) -> Result<(), String> {
    if output.trim() == format!("cargo-mutants {VERSION}") {
        return Ok(());
    }
    let found = output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("no answer");
    Err(format!(
        "cargo-mutants {VERSION} is required, found {found}; run `cargo install --locked cargo-mutants@{VERSION}` inside the repository"
    ))
}

pub(crate) fn require_version(tree: &Path) -> Result<(), String> {
    let mut command = base(tree, &inherited_names());
    command.args(["mutants", "--version"]);
    let out = output(command)?;
    let text = if out.status.success() {
        String::from_utf8_lossy(&out.stdout)
    } else {
        String::from_utf8_lossy(&out.stderr)
    };
    version_matches(&text)
}

#[derive(Clone, Copy)]
pub(crate) struct Spec<'a> {
    pub tree: &'a Path,
    pub config: &'a Path,
    pub package: &'a str,
    pub mode: Mode<'a>,
    pub shard: Option<(u32, u32)>,
    pub in_diff: Option<&'a Path>,
    pub files: &'a [String],
}

#[derive(Clone, Copy)]
pub(crate) enum Mode<'a> {
    List,
    ListNamed {
        names: &'a [String],
    },
    Run {
        test_packages: &'a [String],
        output: &'a Path,
        skip_baseline: bool,
        test_timeout_seconds: u32,
        names: &'a [String],
    },
}

pub(crate) fn command(spec: &Spec) -> Command {
    command_with(spec, &inherited_names())
}

fn inherited_names() -> Vec<OsString> {
    std::env::vars_os().map(|(name, _)| name).collect()
}

pub(crate) fn cargo(tree: &Path) -> Command {
    base(tree, &inherited_names())
}

fn base(tree: &Path, environment: &[OsString]) -> Command {
    let mut command = Command::new("cargo");
    command.current_dir(tree);
    for name in environment {
        let name_text = name.to_string_lossy();
        if SCRUBBED_PREFIXES
            .iter()
            .any(|prefix| name_text.starts_with(prefix))
        {
            command.env_remove(name);
        }
    }
    command.env("CARGO_TERM_COLOR", "never");
    command
}

fn command_with(spec: &Spec, environment: &[OsString]) -> Command {
    let mut command = base(spec.tree, environment);
    command.arg("mutants");
    if let Mode::List | Mode::ListNamed { .. } = spec.mode {
        command.args(["--list", "--json"]);
    }
    command
        .arg("--config")
        .arg(spec.config)
        .args(["--annotations=none", "--no-shuffle", "--sharding=slice"])
        .args(["-p", spec.package]);
    let (index, count) = spec.shard.unwrap_or((0, 1));
    command.arg("--shard").arg(format!("{index}/{count}"));
    if let Some(diff) = spec.in_diff {
        command.arg("--in-diff").arg(diff);
    }
    for file in spec.files {
        command.arg("--file").arg(file);
    }
    if let Mode::ListNamed { names } | Mode::Run { names, .. } = spec.mode {
        for name in names {
            command
                .arg("--re")
                .arg(format!("^{}$", regex::escape(name)));
        }
    }
    if let Mode::Run {
        test_packages,
        output,
        skip_baseline,
        test_timeout_seconds,
        ..
    } = &spec.mode
    {
        command
            .arg("--in-place")
            .arg(if *skip_baseline {
                "--baseline=skip"
            } else {
                "--baseline=run"
            })
            .arg("--timeout")
            .arg(test_timeout_seconds.to_string())
            .arg("--build-timeout")
            .arg(BUILD_TIMEOUT_SECONDS.to_string())
            .arg("--output")
            .arg(output)
            .arg("--test-package")
            .arg(test_packages.join(","));
    }
    command
}

pub(crate) fn output(mut command: Command) -> Result<Output, String> {
    command
        .output()
        .map_err(|e| format!("could not run cargo mutants: {e}; install it and put it on PATH"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn the_version_check_accepts_only_the_pinned_cargo_mutants() {
        let install = "cargo install --locked cargo-mutants@27.1.0";
        let rows: [(&str, Result<(), &str>); 4] = [
            ("cargo-mutants 27.1.0\n", Ok(())),
            ("cargo-mutants 27.1.1\n", Err(install)),
            ("", Err(install)),
            ("error: no such command: `mutants`", Err("no such command")),
        ];
        for (output, expected) in rows {
            match (expected, version_matches(output)) {
                (Ok(()), Ok(())) => {}
                (Err(part), Err(message)) => {
                    assert!(message.contains(part), "{output:?}: {message}")
                }
                (expected, found) => panic!("{output:?}: expected {expected:?}, found {found:?}"),
            }
        }
    }

    fn arguments(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect()
    }

    fn removed(command: &Command) -> Vec<String> {
        let mut names: Vec<String> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn has_pair(arguments: &[String], flag: &str, value: &str) -> bool {
        arguments
            .windows(2)
            .any(|pair| pair[0] == flag && pair[1] == value)
    }

    #[test]
    fn the_cargo_mutants_command_inherits_nothing_from_the_environment() {
        let names: Vec<OsString> = [
            "CARGO_MUTANTS_JOBS",
            "CARGO_MUTANTS_OUTPUT",
            "NEXTEST_RETRIES",
            "NEXTEST_PROFILE",
            "PATH",
        ]
        .map(OsString::from)
        .into();
        let packages = ["a".to_owned(), "b".to_owned()];
        let config = Path::new("/tree/target/mutation/config/listing.toml");
        let list = Spec {
            tree: Path::new("/tree"),
            config,
            package: "a",
            mode: Mode::List,
            shard: None,
            in_diff: None,
            files: &[],
        };
        let run = Spec {
            tree: Path::new("/tree"),
            config,
            package: "a",
            mode: Mode::Run {
                test_packages: &packages,
                output: Path::new("/out"),
                skip_baseline: true,
                test_timeout_seconds: 360,
                names: &[],
            },
            shard: Some((2, 5)),
            in_diff: Some(Path::new("/tree/target/mutation/change.diff")),
            files: &[],
        };
        for spec in [&list, &run] {
            let command = command_with(spec, &names);
            assert_eq!(
                removed(&command),
                [
                    "CARGO_MUTANTS_JOBS",
                    "CARGO_MUTANTS_OUTPUT",
                    "NEXTEST_PROFILE",
                    "NEXTEST_RETRIES"
                ]
            );
            let color = command
                .get_envs()
                .find(|(name, _)| *name == OsStr::new("CARGO_TERM_COLOR"));
            assert_eq!(
                color,
                Some((OsStr::new("CARGO_TERM_COLOR"), Some(OsStr::new("never"))))
            );
            assert_eq!(command.get_current_dir(), Some(Path::new("/tree")));
            let args = arguments(&command);
            assert_eq!(args[0], "mutants");
            assert!(
                has_pair(&args, "--config", config.to_str().unwrap()),
                "{args:?}"
            );
            assert!(has_pair(&args, "-p", "a"), "{args:?}");
            for flag in ["--annotations=none", "--no-shuffle", "--sharding=slice"] {
                assert!(args.iter().any(|a| a == flag), "{flag} in {args:?}");
            }
        }
        let args = arguments(&command_with(&list, &names));
        for flag in ["--list", "--json"] {
            assert!(args.iter().any(|a| a == flag), "{flag} in {args:?}");
        }
        assert!(!args.iter().any(|a| a == "--in-place"), "{args:?}");
        assert!(has_pair(&args, "--shard", "0/1"), "{args:?}");
        let args = arguments(&command_with(&run, &names));
        assert!(!args.iter().any(|a| a == "--list"), "{args:?}");
        for flag in ["--in-place", "--baseline=skip"] {
            assert!(args.iter().any(|a| a == flag), "{flag} in {args:?}");
        }
        for (flag, value) in [
            ("--timeout", "360"),
            ("--build-timeout", "840"),
            ("--output", "/out"),
            ("--test-package", "a,b"),
            ("--shard", "2/5"),
            ("--in-diff", "/tree/target/mutation/change.diff"),
        ] {
            assert!(has_pair(&args, flag, value), "{flag} {value} in {args:?}");
        }
    }

    #[test]
    fn a_named_run_restricts_cargo_mutants_to_those_exact_mutants() {
        let names = [
            "src/a.rs:3:5: replace f::g with (1 + 2) * 3".to_owned(),
            "src/a.rs:9:5: delete ! in h".to_owned(),
        ];
        let packages = ["a".to_owned(), "b".to_owned()];
        let config = Path::new("/tree/target/mutation/config/gate.toml");
        let run = Spec {
            tree: Path::new("/tree"),
            config,
            package: "a",
            mode: Mode::Run {
                test_packages: &packages,
                output: Path::new("/out/0/dependents"),
                skip_baseline: false,
                test_timeout_seconds: 360,
                names: &names,
            },
            shard: Some((0, 1)),
            in_diff: None,
            files: &[],
        };
        let list = Spec {
            mode: Mode::ListNamed { names: &names },
            ..run
        };
        let unrestricted = Spec {
            mode: Mode::Run {
                test_packages: &packages,
                output: Path::new("/out/0"),
                skip_baseline: false,
                test_timeout_seconds: 360,
                names: &[],
            },
            ..run
        };
        let expected = [
            r"^src/a\.rs:3:5: replace f::g with \(1 \+ 2\) \* 3$",
            r"^src/a\.rs:9:5: delete ! in h$",
        ];
        for spec in [&run, &list] {
            let args = arguments(&command_with(spec, &[]));
            let found: Vec<&str> = args
                .windows(2)
                .filter(|pair| pair[0] == "--re")
                .map(|pair| pair[1].as_str())
                .collect();
            assert_eq!(found, expected, "{args:?}");
            assert!(has_pair(&args, "--shard", "0/1"), "{args:?}");
        }
        let args = arguments(&command_with(&unrestricted, &[]));
        assert!(!args.iter().any(|a| a == "--re"), "{args:?}");
    }
}
