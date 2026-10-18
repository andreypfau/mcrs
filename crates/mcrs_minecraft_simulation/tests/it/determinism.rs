use crate::probe::*;
use mcrs_minecraft_simulation::StepOutput;
use mcrs_minecraft_volume::Access;

const SECTIONS: [(i32, i32, i32); 4] = [(0, 0, 0), (1, 0, 0), (2, 0, 0), (3, 0, 0)];

fn rules() -> Vec<Rule> {
    let (p0, p1, q, leased) = (at(1, 1, 1), at(17, 1, 1), at(18, 1, 1), at(49, 1, 1));
    vec![
        Rule::on(
            Hook::BlockTick,
            p0,
            vec![
                Action::ScheduleBlock {
                    pos: p1,
                    block: BLOCK_A,
                    delay: 2,
                },
                Action::ScheduleFluid {
                    pos: q,
                    fluid: WATER,
                    delay: 1,
                },
                Action::QueueEvent {
                    pos: p1,
                    block: BLOCK_A,
                    a: 0,
                    b: 0,
                },
                Action::Set {
                    pos: leased,
                    state: B,
                    flags: 3,
                    limit: 512,
                },
                Action::ScheduleBlock {
                    pos: leased,
                    block: BLOCK_A,
                    delay: 1,
                },
            ],
        )
        .once(),
        Rule::on(
            Hook::BlockEvent,
            p0,
            vec![Action::QueueEvent {
                pos: q,
                block: BLOCK_A,
                a: 1,
                b: 1,
            }],
        )
        .once(),
        Rule::on(
            Hook::FluidTick,
            q,
            vec![Action::Set {
                pos: q,
                state: SOURCE,
                flags: 530,
                limit: 512,
            }],
        )
        .once(),
        Rule::on(
            Hook::BlockTick,
            p1,
            vec![Action::ScheduleBlock {
                pos: p0,
                block: BLOCK_A,
                delay: 1,
            }],
        ),
    ]
}

fn run(reversed: bool) -> (Vec<StepOutput<ProbeKinds>>, Vec<Entry>) {
    let mut order = SECTIONS.to_vec();
    if reversed {
        order.reverse();
    }
    let mut host = Host::new();
    for (x, y, z) in order {
        host.insert(section(x, y, z), A);
    }
    host.put(at(18, 1, 1), FLOWING);
    host.set_access(section(3, 0, 0), Access::Read);
    let mut runner = Runner::new(host, rules());
    let (p0, p1, q) = (at(1, 1, 1), at(17, 1, 1), at(18, 1, 1));
    let first = [
        block_tick_message(0, p0, BLOCK_A, 100),
        block_tick_message(1, p1, BLOCK_A, 101),
        fluid_tick_message(2, q, WATER, 101),
        event_message(3, p0, BLOCK_A),
        set_message(4, at(33, 1, 1), A_OTHER, 3),
    ];
    let mut outputs = vec![runner.level_tick(100, &first)];
    for now in 101..=106 {
        outputs.push(runner.level_tick(now, &[]));
    }
    (outputs, runner.behaviour.log())
}

#[test]
fn two_runs_on_the_same_inputs_give_the_same_writes_and_messages() {
    let (outputs, log) = run(false);
    let (again, again_log) = run(true);
    assert_eq!(outputs, again);
    assert_eq!(log, again_log);

    let work_done = outputs.iter().any(|out| !out.writes.is_empty())
        && outputs.iter().any(|out| !out.outbound.is_empty())
        && outputs.iter().any(|out| !out.delivered.is_empty());
    assert!(
        work_done,
        "the scenario exercises writes, messages and events"
    );
}

#[test]
fn a_step_with_no_inbound_message_and_no_due_work_leaves_the_output_untouched() {
    let mut host = Host::new();
    host.insert(section(0, 0, 0), A);
    let mut runner = Runner::new(host, Vec::new());
    let out = runner.level_tick(100, &[]);
    assert_eq!(out, StepOutput::default());
}
