use mcrs_minecraft_brain::ControlFlags;
use mcrs_minecraft_brain::GoalSpec;
use mcrs_minecraft_brain::Goals;
use mcrs_minecraft_brain::Interrupt;
use mcrs_minecraft_brain::Selector;

#[derive(Debug, PartialEq)]
enum Event {
    Start(usize),
    Stop(usize),
    Tick(usize),
}

struct Script {
    specs: Vec<GoalSpec>,
    startable: Vec<bool>,
    continuable: Vec<bool>,
    events: Vec<Event>,
}

impl Script {
    fn new(specs: Vec<GoalSpec>) -> Self {
        let count = specs.len();
        Self {
            specs,
            startable: vec![false; count],
            continuable: vec![true; count],
            events: Vec::new(),
        }
    }
}

impl Goals for Script {
    fn specs(&self) -> &[GoalSpec] {
        &self.specs
    }
    fn can_start(&mut self, goal: usize) -> bool {
        self.startable.get(goal).copied().unwrap_or(false)
    }
    fn can_continue(&mut self, goal: usize) -> bool {
        self.continuable.get(goal).copied().unwrap_or(false)
    }
    fn start(&mut self, goal: usize) {
        self.events.push(Event::Start(goal));
    }
    fn stop(&mut self, goal: usize) {
        self.events.push(Event::Stop(goal));
    }
    fn tick(&mut self, goal: usize) {
        self.events.push(Event::Tick(goal));
    }
}

#[test]
fn first_of_two_equal_goals_wins_and_the_second_never_interrupts_it() {
    let mut script = Script::new(vec![
        GoalSpec::new(4, ControlFlags::MOVE),
        GoalSpec::new(4, ControlFlags::MOVE),
    ]);
    script.startable = vec![true, true];
    let mut selector = Selector::default();
    selector.evaluate(&mut script, ControlFlags::NONE);
    selector.evaluate(&mut script, ControlFlags::NONE);
    assert_eq!(
        script.events,
        [Event::Start(0), Event::Tick(0), Event::Tick(0)]
    );
    assert_eq!(selector.holder(0), Some(0));
}

#[test]
fn a_smaller_priority_number_interrupts_after_the_stop_of_the_holder() {
    let mut script = Script::new(vec![
        GoalSpec::new(1, ControlFlags::MOVE),
        GoalSpec::new(6, ControlFlags::MOVE),
        GoalSpec::new(8, ControlFlags::MOVE.union(ControlFlags::LOOK)),
        GoalSpec::new(7, ControlFlags::LOOK),
    ]);
    let mut selector = Selector::default();
    script.startable = vec![false, false, true, false];
    selector.evaluate(&mut script, ControlFlags::NONE);
    script.events.clear();

    script.startable = vec![true, true, false, true];
    selector.evaluate(&mut script, ControlFlags::NONE);
    assert_eq!(
        script.events,
        [
            Event::Stop(2),
            Event::Start(0),
            Event::Start(3),
            Event::Tick(0),
            Event::Tick(3)
        ],
        "the first goal stops the look-around and takes move; the stroll waits behind it; the look \
         goal takes the released look"
    );
    assert!(!selector.is_running(1));
}

#[test]
fn a_goal_stopped_by_an_interrupt_keeps_its_other_flags_until_the_next_evaluation() {
    let mut script = Script::new(vec![
        GoalSpec::new(1, ControlFlags::MOVE),
        GoalSpec::new(4, ControlFlags::MOVE.union(ControlFlags::LOOK)),
        GoalSpec::new(7, ControlFlags::LOOK),
    ]);
    let mut selector = Selector::default();
    script.startable = vec![false, true, false];
    selector.evaluate(&mut script, ControlFlags::NONE);
    script.events.clear();

    script.startable = vec![true, false, true];
    selector.evaluate(&mut script, ControlFlags::NONE);
    assert_eq!(
        script.events,
        [Event::Stop(1), Event::Start(0), Event::Tick(0)],
        "the panic takes only move from the tempt; the tempt still names look, so the look goal \
         is not examined (GOAL-FRM-002)"
    );
    assert_eq!(selector.holder(1), Some(1));
    assert_eq!(selector.released().holder(1), None);

    script.events.clear();
    selector.evaluate(&mut script, ControlFlags::NONE);
    assert_eq!(
        script.events,
        [Event::Start(2), Event::Tick(0), Event::Tick(2)]
    );
}

#[test]
fn a_refused_interrupt_and_a_disabled_flag_block_the_start() {
    let mut refusing = GoalSpec::new(5, ControlFlags::MOVE);
    refusing.interrupt = Interrupt::Refused;
    let mut script = Script::new(vec![GoalSpec::new(1, ControlFlags::MOVE), refusing]);
    let mut selector = Selector::default();
    script.startable = vec![false, true];
    selector.evaluate(&mut script, ControlFlags::NONE);
    script.startable = vec![true, true];
    selector.evaluate(&mut script, ControlFlags::NONE);
    assert!(selector.is_running(1) && !selector.is_running(0));

    selector.evaluate(&mut script, ControlFlags::MOVE);
    assert!(!selector.is_running(0) && !selector.is_running(1));
    assert_eq!(script.events.last(), Some(&Event::Stop(1)));
}

#[test]
fn a_goal_with_no_flag_runs_beside_the_holders() {
    let mut script = Script::new(vec![
        GoalSpec::new(1, ControlFlags::MOVE.union(ControlFlags::LOOK)),
        GoalSpec::new(5, ControlFlags::NONE),
    ]);
    script.startable = vec![true, true];
    let mut selector = Selector::default();
    selector.evaluate(&mut script, ControlFlags::NONE);
    assert!(selector.is_running(0) && selector.is_running(1));
}
