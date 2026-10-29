use crate::{Builder, OPEN, TestGraph};
use mcrs_command::{
    Chain, End, Executor, Failure, ForkMode, Limits, NodeId, ParseScratch, SourceLists, execute,
};

const SAY: u32 = 0;
const BAD: u32 = 1;
const FORK: u32 = 10;
const SINGLE: u32 = 11;
const FAIL: u32 = 12;
const FORK_FAIL: u32 = 13;
const DROP: u32 = 14;

fn graph() -> TestGraph {
    let mut b = Builder::new();
    let x = b.literal(NodeId::ROOT, "x", OPEN).unwrap();
    for (name, modifier, mode) in [
        ("fork", FORK, ForkMode::Fork),
        ("single", SINGLE, ForkMode::Single),
        ("fail", FAIL, ForkMode::Single),
        ("forkfail", FORK_FAIL, ForkMode::Fork),
        ("drop", DROP, ForkMode::Fork),
    ] {
        let node = b.literal(x, name, OPEN).unwrap();
        b.redirect(node, x).unwrap();
        b.modifies(node, modifier, mode).unwrap();
    }
    let run = b.literal(x, "run", OPEN).unwrap();
    b.redirect(run, NodeId::ROOT).unwrap();
    for (name, command) in [("say", SAY), ("bad", BAD)] {
        let node = b.literal(NodeId::ROOT, name, OPEN).unwrap();
        b.executes(node, command).unwrap();
    }
    b.freeze().unwrap()
}

/// Records what the chain asks for, in order. The source `4` fails every modifier.
#[derive(Default)]
struct Log(Vec<String>);

impl Executor<i32, u32> for Log {
    type Error = i32;

    fn modify(
        &mut self,
        _: usize,
        modifier: &u32,
        source: &i32,
        out: &mut Vec<i32>,
    ) -> Result<(), i32> {
        self.0.push(format!("modify {source}"));
        match *modifier {
            FORK if *source != 4 => out.extend([source * 10 + 1, source * 10 + 2]),
            SINGLE if *source != 4 => out.push(source + 1),
            DROP => {}
            _ => return Err(*source),
        }
        Ok(())
    }

    fn run(&mut self, _: usize, command: &u32, source: &i32) -> Result<i32, i32> {
        self.0.push(format!("run {source}"));
        if *command == BAD {
            Err(*source)
        } else {
            Ok(*source)
        }
    }

    fn report(&mut self, failure: Failure<i32>) {
        self.0.push(match failure {
            Failure::Command(source) => format!("report {source}"),
            Failure::ForkLimit(limit) => format!("fork limit {limit}"),
        });
    }
}

fn run(line: &str, source: i32, limits: Limits) -> (End, Vec<String>) {
    let graph = graph();
    let mut scratch = ParseScratch::default();
    let parse = graph.parse(line, 0, &0u8, &mut scratch);
    parse.check(&graph).unwrap();
    let mut chain = Chain::default();
    chain.copy_from(&graph, &parse);
    let mut log = Log::default();
    let end = execute(
        chain.stages(),
        source,
        limits,
        &mut log,
        &mut SourceLists::default(),
    );
    (end, log.0)
}

#[test]
fn a_chain_runs_each_stage_over_the_whole_list_and_reports_only_outside_a_fork() {
    let unlimited = Limits::default();
    let cases: [(&str, i32, End, &[&str]); 9] = [
        (
            "x fork fork run say",
            1,
            End::Completed,
            &[
                "modify 1",
                "modify 11",
                "modify 12",
                "run 111",
                "run 112",
                "run 121",
                "run 122",
            ],
        ),
        (
            "x single run say",
            1,
            End::Completed,
            &["modify 1", "run 2"],
        ),
        (
            "x single run bad",
            1,
            End::Completed,
            &["modify 1", "run 2", "report 2"],
        ),
        (
            "x fork run bad",
            1,
            End::Completed,
            &["modify 1", "run 11", "run 12"],
        ),
        (
            "x single fail run say",
            3,
            End::Failed,
            &["modify 3", "modify 4", "report 4"],
        ),
        (
            "x fork single run say",
            0,
            End::Completed,
            &["modify 0", "modify 1", "modify 2", "run 2", "run 3"],
        ),
        (
            "x single forkfail run say",
            3,
            End::Completed,
            &["modify 3", "modify 4"],
        ),
        (
            "x fork single single single run say",
            0,
            End::Completed,
            &[
                "modify 0", "modify 1", "modify 2", "modify 2", "modify 3", "modify 3", "modify 4",
                "run 4",
            ],
        ),
        ("x drop single run say", 0, End::Completed, &["modify 0"]),
    ];
    for (line, source, end, log) in cases {
        assert_eq!(
            run(line, source, unlimited),
            (end, log.iter().map(|s| s.to_string()).collect()),
            "{line}"
        );
    }
}

#[test]
fn the_fork_and_sequence_limits_stop_the_chain() {
    let forks = |forks| Limits {
        forks,
        ..Limits::default()
    };
    let sequence = |sequence| Limits {
        sequence,
        ..Limits::default()
    };
    let cases: [(&str, Limits, End, &[&str]); 5] = [
        (
            "x single run say",
            forks(1),
            End::ForkLimit,
            &["modify 1", "fork limit 1"],
        ),
        ("x fork run say", forks(2), End::ForkLimit, &["modify 1"]),
        (
            "x fork run say",
            forks(3),
            End::Completed,
            &["modify 1", "run 11", "run 12"],
        ),
        (
            "x fork fork run say",
            sequence(3),
            End::SequenceLimit,
            &["modify 1", "modify 11", "modify 12", "run 111"],
        ),
        (
            "x fork run say",
            sequence(0),
            End::SequenceLimit,
            &["modify 1"],
        ),
    ];
    for (line, limits, end, log) in cases {
        assert_eq!(
            run(line, 1, limits),
            (end, log.iter().map(|s| s.to_string()).collect()),
            "{line} {limits:?}"
        );
    }
}
