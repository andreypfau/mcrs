#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Check {
    Size,
    Suppressions,
    Bypasses,
    Parity,
    UnusedDeps,
    Fmt,
    Clippy,
    Test,
    NoBevy,
    Wasm32,
    ExportedBuild,
}

impl Check {
    pub const fn name(self) -> &'static str {
        match self {
            Check::Size => "size",
            Check::Suppressions => "suppressions",
            Check::Bypasses => "bypasses",
            Check::Parity => "parity",
            Check::UnusedDeps => "unused-deps",
            Check::Fmt => "fmt",
            Check::Clippy => "clippy",
            Check::Test => "test",
            Check::NoBevy => "no-bevy",
            Check::Wasm32 => "wasm32",
            Check::ExportedBuild => "exported-build",
        }
    }

    #[cfg(test)]
    pub const fn is_git_only(self) -> bool {
        matches!(
            self,
            Check::Size | Check::Suppressions | Check::Bypasses | Check::Parity
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trigger {
    Edit,
    Commit,
    Push,
    Ci,
    Full,
    Gpu,
}

impl Trigger {
    pub const ALL: [Trigger; 6] = [
        Trigger::Edit,
        Trigger::Commit,
        Trigger::Push,
        Trigger::Ci,
        Trigger::Full,
        Trigger::Gpu,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Trigger::Edit => "edit",
            Trigger::Commit => "commit",
            Trigger::Push => "push",
            Trigger::Ci => "ci",
            Trigger::Full => "full",
            Trigger::Gpu => "gpu",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Trigger::Edit => {
                "after a file is written: change size, lint suppressions, test bypasses, fixture parity, formatting"
            }
            Trigger::Commit => {
                "before a commit: change size, lint suppressions, test bypasses, fixture parity, unused dependencies, formatting"
            }
            Trigger::Push => {
                "before a push: the commit checks; ci runs clippy, the tests and the builds"
            }
            Trigger::Ci => "on every pull request and every push to main: all checks",
            Trigger::Full => "the whole test suite, exhaustive sweeps included",
            Trigger::Gpu => "the tests that need a Metal GPU",
        }
    }

    pub const fn is_local(self) -> bool {
        match self {
            Trigger::Edit | Trigger::Commit | Trigger::Push => true,
            Trigger::Ci | Trigger::Full | Trigger::Gpu => false,
        }
    }

    #[cfg(test)]
    pub const fn runs_on_every_pull_request(self) -> bool {
        matches!(self, Trigger::Ci)
    }

    pub fn parse(name: &str) -> Option<Trigger> {
        Trigger::ALL.into_iter().find(|t| t.name() == name)
    }
}

pub const fn checks(trigger: Trigger) -> &'static [Check] {
    use Check::*;
    match trigger {
        Trigger::Edit => &[Size, Suppressions, Bypasses, Parity, Fmt],
        Trigger::Commit | Trigger::Push => &[Size, Suppressions, Bypasses, Parity, UnusedDeps, Fmt],
        Trigger::Ci => &[
            Size,
            Suppressions,
            Bypasses,
            Parity,
            UnusedDeps,
            Fmt,
            Clippy,
            Test,
            NoBevy,
            Wasm32,
            ExportedBuild,
        ],
        Trigger::Full | Trigger::Gpu => &[Test],
    }
}

pub fn from_args(args: &[String]) -> Result<Trigger, String> {
    match args {
        [] => Err("missing trigger".to_owned()),
        [name] if name.is_empty() => Err("the trigger is empty".to_owned()),
        [name] => Trigger::parse(name).ok_or_else(|| format!("unknown trigger {name:?}")),
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
            text.push_str(&format!(
                "  {:<15}{}\n",
                trigger.name(),
                trigger.description()
            ));
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
            .filter(|t| t.is_local())
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
        assert_eq!(every, [Trigger::Ci]);
    }

    #[test]
    fn the_subset_test_fails_on_a_check_no_pull_request_trigger_lists_whatever_the_order() {
        fn table(trigger: Trigger) -> &'static [Check] {
            match trigger {
                Trigger::Ci => &[Check::Test, Check::Fmt],
                Trigger::Push => &[Check::Clippy, Check::Fmt, Check::Test],
                Trigger::Commit => &[Check::Fmt, Check::Wasm32],
                _ => &[Check::Fmt],
            }
        }
        assert_eq!(
            outside_pull_requests(table),
            [
                (Trigger::Commit, Check::Wasm32),
                (Trigger::Push, Check::Clippy)
            ]
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
        for name in ["edit", "commit", "push"] {
            assert!(listed(here, name), "{name} is not listed for this machine");
            assert!(!listed(elsewhere, name), "{name} is listed apart");
        }
        for name in ["ci", "full", "gpu"] {
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
            let found = from_args(&args).ok();
            assert_eq!(found, expected, "{args:?}");
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
        let carries_it = [Trigger::Edit, Trigger::Commit, Trigger::Push, Trigger::Ci];
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
}
