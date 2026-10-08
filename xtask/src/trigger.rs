#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Check {
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
            Check::Fmt => "fmt",
            Check::Clippy => "clippy",
            Check::Test => "test",
            Check::NoBevy => "no-bevy",
            Check::Wasm32 => "wasm32",
            Check::ExportedBuild => "exported-build",
        }
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
            Trigger::Edit => "after a file is written: formatting of the changed Rust files",
            Trigger::Commit => "before a commit: formatting, and clippy on the changed crates",
            Trigger::Push => "before a push: every check that ci runs",
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

    pub fn parse(name: &str) -> Option<Trigger> {
        Trigger::ALL.into_iter().find(|t| t.name() == name)
    }
}

pub const fn checks(trigger: Trigger) -> &'static [Check] {
    use Check::*;
    match trigger {
        Trigger::Edit => &[Fmt],
        Trigger::Commit => &[Fmt, Clippy],
        Trigger::Push | Trigger::Ci => &[Fmt, Clippy, Test, NoBevy, Wasm32, ExportedBuild],
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
                "  {:<7}{}\n",
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

    fn outside_ci(table: impl Fn(Trigger) -> &'static [Check]) -> Vec<(Trigger, Check)> {
        Trigger::ALL
            .into_iter()
            .filter(|t| t.is_local())
            .flat_map(|t| table(t).iter().map(move |&c| (t, c)))
            .filter(|&(_, c)| !table(Trigger::Ci).contains(&c))
            .collect()
    }

    #[test]
    fn every_local_trigger_runs_a_subset_of_ci() {
        assert_eq!(outside_ci(checks), []);
        let local: Vec<_> = Trigger::ALL.into_iter().filter(|t| t.is_local()).collect();
        assert!(local.len() >= 3, "{local:?}");
    }

    #[test]
    fn the_subset_test_fails_on_a_check_ci_lacks_whatever_the_order() {
        fn table(trigger: Trigger) -> &'static [Check] {
            match trigger {
                Trigger::Ci => &[Check::Test, Check::Fmt],
                Trigger::Push => &[Check::Fmt, Check::Test],
                Trigger::Commit => &[Check::Fmt, Check::Wasm32],
                _ => &[Check::Fmt],
            }
        }
        assert_eq!(outside_ci(table), [(Trigger::Commit, Check::Wasm32)]);
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
            assert_eq!(from_args(&args).ok(), expected, "{args:?}");
        }
    }
}
