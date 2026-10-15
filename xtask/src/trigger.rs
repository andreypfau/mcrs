#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Check {
    Size,
    GateConfig,
    Suppressions,
    Bypasses,
    MutationMarker,
    Floors,
    Parity,
    Clones,
    UnusedDeps,
    MutationConfig,
    Fmt,
    Clippy,
    Test,
    NoBevy,
    Wasm32,
    ExportedBuild,
    Mutation,
}

impl Check {
    pub const fn name(self) -> &'static str {
        match self {
            Check::Size => "size",
            Check::GateConfig => "gate-config",
            Check::Suppressions => "suppressions",
            Check::Bypasses => "bypasses",
            Check::MutationMarker => "mutation-marker",
            Check::Floors => "floors",
            Check::Parity => "parity",
            Check::Clones => "clones",
            Check::UnusedDeps => "unused-deps",
            Check::MutationConfig => "mutation-config",
            Check::Fmt => "fmt",
            Check::Clippy => "clippy",
            Check::Test => "test",
            Check::NoBevy => "no-bevy",
            Check::Wasm32 => "wasm32",
            Check::ExportedBuild => "exported-build",
            Check::Mutation => "mutation",
        }
    }

    #[cfg(test)]
    pub const fn runs(self) -> Runs {
        match self {
            Check::Size
            | Check::GateConfig
            | Check::Suppressions
            | Check::Bypasses
            | Check::MutationMarker
            | Check::Floors
            | Check::Parity => Runs::GitData,
            Check::Clones => Runs::PinnedToolOverExportedFiles,
            Check::UnusedDeps
            | Check::MutationConfig
            | Check::Fmt
            | Check::Clippy
            | Check::Test
            | Check::NoBevy
            | Check::Wasm32
            | Check::ExportedBuild
            | Check::Mutation => Runs::Cargo,
        }
    }

    #[cfg(test)]
    pub const fn is_git_only(self) -> bool {
        matches!(self.runs(), Runs::GitData)
    }
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Runs {
    GitData,
    PinnedToolOverExportedFiles,
    Cargo,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trigger {
    Edit,
    Commit,
    Push,
    Ci,
    Full,
    Gpu,
    Policy,
    Mutation,
    MutationLocal,
}

impl Trigger {
    pub const ALL: [Trigger; 9] = [
        Trigger::Edit,
        Trigger::Commit,
        Trigger::Push,
        Trigger::Ci,
        Trigger::Full,
        Trigger::Gpu,
        Trigger::Policy,
        Trigger::Mutation,
        Trigger::MutationLocal,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Trigger::Edit => "edit",
            Trigger::Commit => "commit",
            Trigger::Push => "push",
            Trigger::Ci => "ci",
            Trigger::Full => "full",
            Trigger::Gpu => "gpu",
            Trigger::Policy => "policy",
            Trigger::Mutation => "mutation",
            Trigger::MutationLocal => "mutation-local",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Trigger::Edit => {
                "after a file is written: change size, lint suppressions, test bypasses, fixture parity, formatting"
            }
            Trigger::Commit => {
                "before a commit: change size, gate configuration, lint suppressions, test bypasses, fixture parity, unused dependencies, formatting"
            }
            Trigger::Push => {
                "before a push: the commit checks and the clone scan; ci runs clippy, the tests and the builds"
            }
            Trigger::Ci => "on every pull request and every push to main: all checks",
            Trigger::Full => "the whole test suite, exhaustive sweeps included",
            Trigger::Gpu => "the tests that need a Metal GPU",
            Trigger::Policy => {
                "in CI from the default branch: the git-only checks and the clone scan on a pull request head read as data"
            }
            Trigger::Mutation => {
                "by hand only, never in a hook or in CI: mutation markers, floors and configuration, then the surviving mutants in changed code, from shards run beforehand"
            }
            Trigger::MutationLocal => {
                "on demand on this machine: the surviving mutants in changed code"
            }
        }
    }

    pub const fn is_local(self) -> bool {
        match self {
            Trigger::Edit | Trigger::Commit | Trigger::Push | Trigger::MutationLocal => true,
            Trigger::Ci | Trigger::Full | Trigger::Gpu | Trigger::Policy | Trigger::Mutation => {
                false
            }
        }
    }

    #[cfg(test)]
    pub const fn runs_on_every_pull_request(self) -> bool {
        matches!(self, Trigger::Ci | Trigger::Policy)
    }

    pub fn parse(name: &str) -> Option<Trigger> {
        Trigger::ALL.into_iter().find(|t| t.name() == name)
    }
}

pub const fn checks(trigger: Trigger) -> &'static [Check] {
    use Check::*;
    match trigger {
        Trigger::Edit => &[Size, Suppressions, Bypasses, Parity, Fmt],
        Trigger::Commit => &[
            Size,
            GateConfig,
            Suppressions,
            Bypasses,
            Parity,
            UnusedDeps,
            Fmt,
        ],
        Trigger::Push => &[
            Size,
            GateConfig,
            Suppressions,
            Bypasses,
            Parity,
            Clones,
            UnusedDeps,
            Fmt,
        ],
        Trigger::Ci => &[
            Size,
            GateConfig,
            Suppressions,
            Bypasses,
            Parity,
            Clones,
            UnusedDeps,
            Fmt,
            Clippy,
            Test,
            NoBevy,
            Wasm32,
            ExportedBuild,
        ],
        Trigger::Full | Trigger::Gpu => &[Test],
        Trigger::Policy => &[Size, GateConfig, Suppressions, Bypasses, Parity, Clones],
        Trigger::Mutation => &[MutationMarker, Floors, MutationConfig, Mutation],
        Trigger::MutationLocal => &[Mutation],
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
    pub trigger: Trigger,
    pub head: Option<String>,
}

pub fn from_args(args: &[String]) -> Result<Invocation, String> {
    let plain = |trigger| Invocation {
        trigger,
        head: None,
    };
    match args {
        [] => Err("missing trigger".to_owned()),
        [name] if name.is_empty() => Err("the trigger is empty".to_owned()),
        [name] => match Trigger::parse(name) {
            Some(Trigger::Policy) => Err("policy needs the head commit: policy <head>".to_owned()),
            Some(trigger) => Ok(plain(trigger)),
            None => Err(format!("unknown trigger {name:?}")),
        },
        [name, head] if name == Trigger::Policy.name() => match head.as_str() {
            "" => Err("the head commit is empty".to_owned()),
            option if option.starts_with('-') => {
                Err(format!("the head must be a commit id, not {option:?}"))
            }
            _ => Ok(Invocation {
                trigger: Trigger::Policy,
                head: Some(head.clone()),
            }),
        },
        more => Err(format!(
            "expected exactly one trigger, got {}: {}",
            more.len(),
            more.join(" ")
        )),
    }
}

pub fn usage() -> String {
    let mut text = String::from("usage: run-checks <trigger>\n");
    for (heading, local) in [
        ("triggers that run on this machine", true),
        ("triggers that run in CI or on demand", false),
    ] {
        text.push_str(&format!("\n{heading}:\n"));
        for trigger in Trigger::ALL.into_iter().filter(|t| t.is_local() == local) {
            let usage = match trigger {
                Trigger::Policy => "policy <head>",
                other => other.name(),
            };
            text.push_str(&format!("  {usage:<15}{}\n", trigger.description()));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outside_pull_requests(table: impl Fn(Trigger) -> &'static [Check]) -> Vec<(Trigger, Check)> {
        let on_pull_requests: Vec<Check> = Trigger::ALL
            .into_iter()
            .filter(|t| t.runs_on_every_pull_request())
            .flat_map(|t| table(t).iter().copied())
            .collect();
        Trigger::ALL
            .into_iter()
            .filter(|&t| t.is_local() && t != Trigger::MutationLocal)
            .flat_map(|t| table(t).iter().map(move |&c| (t, c)))
            .filter(|(_, c)| !on_pull_requests.contains(c))
            .collect()
    }

    #[test]
    fn every_local_trigger_runs_a_subset_of_the_pull_request_checks() {
        assert_eq!(outside_pull_requests(checks), []);
        let local: Vec<_> = Trigger::ALL.into_iter().filter(|t| t.is_local()).collect();
        assert!(local.len() >= 3, "{local:?}");
        let every: Vec<_> = Trigger::ALL
            .into_iter()
            .filter(|t| t.runs_on_every_pull_request())
            .collect();
        assert_eq!(every, [Trigger::Ci, Trigger::Policy]);
    }

    #[test]
    fn the_subset_test_fails_on_a_check_no_pull_request_trigger_lists_whatever_the_order() {
        fn table(trigger: Trigger) -> &'static [Check] {
            match trigger {
                Trigger::Ci => &[Check::Test, Check::Fmt],
                Trigger::Push => &[Check::Mutation, Check::Fmt, Check::Test],
                Trigger::Commit => &[Check::Fmt, Check::Wasm32],
                _ => &[Check::Fmt],
            }
        }
        assert_eq!(
            outside_pull_requests(table),
            [
                (Trigger::Commit, Check::Wasm32),
                (Trigger::Push, Check::Mutation)
            ]
        );
    }

    #[test]
    fn every_mutation_check_runs_only_from_the_manual_mutation_triggers_which_no_hook_and_no_pull_request_runs()
     {
        let mutation_checks = [
            Check::MutationMarker,
            Check::Floors,
            Check::MutationConfig,
            Check::Mutation,
        ];
        let manual = [Trigger::Mutation, Trigger::MutationLocal];
        assert_eq!(checks(Trigger::Mutation), mutation_checks);
        assert_eq!(checks(Trigger::MutationLocal), [Check::Mutation]);
        assert!(!Trigger::Mutation.is_local());
        for trigger in manual {
            assert!(!trigger.runs_on_every_pull_request(), "{trigger:?}");
        }
        for trigger in Trigger::ALL.into_iter().filter(|t| !manual.contains(t)) {
            for check in mutation_checks {
                assert!(!checks(trigger).contains(&check), "{trigger:?} {check:?}");
            }
        }
        assert_eq!(
            Trigger::parse("mutation-local"),
            Some(Trigger::MutationLocal)
        );
    }

    #[test]
    fn usage_lists_the_hook_and_on_demand_triggers_under_this_machine_and_the_rest_apart() {
        let text = usage();
        let (here, elsewhere) = text
            .split_once("triggers that run in CI or on demand:")
            .expect("both headings");
        let listed = |section: &str, name: &str| {
            section
                .lines()
                .any(|line| line.trim_start().split(' ').next() == Some(name))
        };
        for name in ["edit", "commit", "push", "mutation-local"] {
            assert!(listed(here, name), "{name} is not listed for this machine");
            assert!(!listed(elsewhere, name), "{name} is listed apart");
        }
        for name in ["ci", "full", "gpu", "policy", "mutation"] {
            assert!(listed(elsewhere, name), "{name} is not listed apart");
            assert!(!listed(here, name), "{name} is listed for this machine");
        }
    }

    #[test]
    fn every_trigger_lists_a_check_and_round_trips_its_name() {
        for trigger in Trigger::ALL {
            assert!(!checks(trigger).is_empty(), "{trigger:?} lists no check");
            assert_eq!(Trigger::parse(trigger.name()), Some(trigger));
        }
        let mut names: Vec<_> = Trigger::ALL.iter().map(|t| t.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Trigger::ALL.len());
    }

    #[test]
    fn arguments_other_than_one_known_trigger_are_refused() {
        let cases: [(&[&str], Option<Trigger>); 7] = [
            (&[], None),
            (&[""], None),
            (&["bogus"], None),
            (&["Edit"], None),
            (&["edit", "ci"], None),
            (&["edit"], Some(Trigger::Edit)),
            (&["gpu"], Some(Trigger::Gpu)),
        ];
        for (args, expected) in cases {
            let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
            let found = from_args(&args).ok().map(|invocation| invocation.trigger);
            assert_eq!(found, expected, "{args:?}");
        }
    }

    #[test]
    fn only_policy_takes_a_head_and_it_must_be_a_commit_id() {
        let cases: [(&[&str], Option<&str>); 8] = [
            (&["policy", "abc123"], Some("abc123")),
            (&["policy"], None),
            (&["policy", ""], None),
            (&["policy", "--output=x"], None),
            (&["policy", "a", "b"], None),
            (&["edit", "abc123"], None),
            (&["ci", "abc123"], None),
            (&["push", "abc123"], None),
        ];
        for (args, head) in cases {
            let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
            let found = from_args(&args).ok().and_then(|invocation| invocation.head);
            assert_eq!(found.as_deref(), head, "{args:?}");
        }
    }

    #[test]
    fn every_trigger_lists_the_git_only_checks_first() {
        for trigger in Trigger::ALL {
            let listed = checks(trigger);
            let first_other = listed.iter().position(|check| !check.is_git_only());
            if let Some(first_other) = first_other {
                let late: Vec<_> = listed[first_other..]
                    .iter()
                    .filter(|check| check.is_git_only())
                    .collect();
                assert!(
                    late.is_empty(),
                    "{trigger:?} lists {late:?} after another check"
                );
            }
        }
    }

    #[test]
    fn parity_is_the_last_git_only_check_of_every_trigger_that_runs_on_a_pull_request_or_locally() {
        let carries_it = [
            Trigger::Edit,
            Trigger::Commit,
            Trigger::Push,
            Trigger::Ci,
            Trigger::Policy,
        ];
        for trigger in Trigger::ALL {
            let listed = checks(trigger);
            let position = listed.iter().position(|&c| c == Check::Parity);
            if !carries_it.contains(&trigger) {
                assert_eq!(position, None, "{trigger:?}");
                continue;
            }
            let position = position.unwrap_or_else(|| panic!("{trigger:?} lacks the check"));
            assert!(Check::Parity.is_git_only());
            assert!(
                listed[position + 1..]
                    .iter()
                    .all(|check| !check.is_git_only()),
                "{trigger:?} lists a git-only check after parity"
            );
            assert_eq!(listed.iter().filter(|&&c| c == Check::Parity).count(), 1);
        }
    }

    #[test]
    fn every_check_and_trigger_has_its_own_name_and_description() {
        let mut checks: Vec<Check> = Trigger::ALL
            .into_iter()
            .flat_map(|t| checks(t).to_vec())
            .collect();
        checks.sort_by_key(|check| check.name());
        checks.dedup();
        let names: Vec<&str> = checks.iter().map(|check| check.name()).collect();
        let mut unique = names.clone();
        unique.dedup();
        assert_eq!(names, unique);
        assert!(names.iter().all(|name| !name.is_empty()));
        assert!(names.contains(&"parity"));

        let mut descriptions: Vec<&str> = Trigger::ALL.iter().map(|t| t.description()).collect();
        assert!(descriptions.iter().all(|text| text.len() > 20));
        descriptions.sort_unstable();
        descriptions.dedup();
        assert_eq!(descriptions.len(), Trigger::ALL.len());
        for trigger in [Trigger::Edit, Trigger::Commit] {
            assert!(
                trigger.description().contains("fixture parity"),
                "{trigger:?}"
            );
        }
    }

    #[test]
    fn the_base_run_trigger_builds_and_runs_nothing_from_the_head() {
        let listed = checks(Trigger::Policy);
        let cargo: Vec<_> = listed
            .iter()
            .filter(|check| check.runs() == Runs::Cargo)
            .collect();
        assert!(
            cargo.is_empty(),
            "the policy trigger runs {cargo:?}, which would run cargo on the head"
        );
        let tools: Vec<_> = listed
            .iter()
            .filter(|check| check.runs() == Runs::PinnedToolOverExportedFiles)
            .collect();
        assert_eq!(tools, [&Check::Clones]);
        for check in [
            Check::UnusedDeps,
            Check::Fmt,
            Check::Clippy,
            Check::Test,
            Check::NoBevy,
            Check::Wasm32,
            Check::ExportedBuild,
        ] {
            assert!(!listed.contains(&check), "policy lists {check:?}");
        }
    }

    #[test]
    fn policy_runs_a_subset_of_ci() {
        let outside: Vec<_> = checks(Trigger::Policy)
            .iter()
            .filter(|check| !checks(Trigger::Ci).contains(check))
            .collect();
        assert!(outside.is_empty(), "{outside:?}");
    }
}
