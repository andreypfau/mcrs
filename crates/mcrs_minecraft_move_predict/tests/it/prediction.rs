//! Layer 2 against a scripted client: the client runs the physics model on its own side with
//! the changes it has applied, as a vanilla client does, and reports the result. An honest
//! client must pass; a client that changes its report must not.

use glam::DVec3;
use mcrs_minecraft_move_predict::Change;
use mcrs_minecraft_move_predict::ChangeStates;
use mcrs_minecraft_move_predict::ClientTick;
use mcrs_minecraft_move_predict::Limits;
use mcrs_minecraft_move_predict::Mode;
use mcrs_minecraft_move_predict::Outcome;
use mcrs_minecraft_move_predict::Prediction;
use mcrs_minecraft_move_predict::Pusher;
use mcrs_minecraft_move_validate::Move;
use mcrs_minecraft_physics::Keys;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::Orientation;
use mcrs_minecraft_physics::PlayerInput;
use mcrs_minecraft_physics::PlayerState;
use mcrs_minecraft_physics::Relative;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::Teleport;
use mcrs_minecraft_physics::TestFixture;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::apply_teleport;
use mcrs_minecraft_physics::push_vector;
use mcrs_minecraft_physics::tick_player;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

const FORWARD: Keys = Keys {
    forward: true,
    back: false,
    left: false,
    right: false,
    jump: false,
    shift: false,
    sprint: false,
    auto_jump: false,
};

/// A server with layer 2 for one player, and the client of that player.
struct Session {
    fixture: TestFixture,
    client_world: TestWorld,
    client: PlayerState,
    client_applied: usize,
    changes: Vec<Change>,
    prediction: Prediction,
    pushers: Vec<Pusher>,
    input: PlayerInput,
    /// The sprint state, the flight state and the item use that the tick packets change.
    sprint: Option<bool>,
    flight: Option<bool>,
    item_use: Option<bool>,
    scratch: Scratch,
}

impl Session {
    /// A player that stands on a floor of stone at `y = 0`, after it has settled.
    fn new() -> Self {
        let mut fixture = TestFixture::new();
        let stone = fixture.blocks.stone;
        fixture.world.fill(
            BlockPos::new(-20, -1, -20),
            BlockPos::new(20, -1, 20),
            stone,
        );
        let state = PlayerState::new(DVec3::new(0.5, 0.0, 0.5));
        let mut session = Self {
            client_world: fixture.world.clone(),
            fixture,
            client: state,
            client_applied: 0,
            changes: Vec::new(),
            prediction: Prediction::new(state, Limits::default()),
            pushers: Vec::new(),
            input: PlayerInput {
                rotation: Orientation::facing(-90.0, 0.0),
                ..PlayerInput::default()
            },
            sprint: None,
            flight: None,
            item_use: None,
            scratch: Scratch::new(),
        };
        for _ in 0..3 {
            session.step(Keys::default());
        }
        session
    }

    /// The server writes a change; the client has it once a tick applies it.
    fn send(&mut self, change: Change) {
        if let Change::Block { at, new, .. } = change {
            self.fixture.world.set(at, new);
        }
        self.prediction.change(change);
        self.changes.push(change);
    }

    /// One honest client tick in which every change sent is applied.
    fn step(&mut self, keys: Keys) -> Outcome {
        let all = self.changes.len() as u64;
        let states = ChangeStates {
            applied: all,
            seen: all,
        };
        self.tick(keys, states, self.changes.len(), DVec3::ZERO, |_| {})
    }

    /// One client tick: the client applies the changes up to `client_applies`, adds `push` and
    /// runs its tick; `edit` changes its report.
    fn tick(
        &mut self,
        keys: Keys,
        states: ChangeStates,
        client_applies: usize,
        push: DVec3,
        edit: impl FnOnce(&mut Move),
    ) -> Outcome {
        let new = self
            .changes
            .get(self.client_applied..client_applies)
            .unwrap_or(&[]);
        for change in new {
            if let Change::Block { at, new, .. } = *change {
                self.client_world.set(at, new);
            }
            apply(change, &mut self.client);
        }
        self.client_applied = self.client_applied.max(client_applies);
        self.input.keys = keys;
        let start = self.client;
        let input = PlayerInput { push, ..self.input };
        let view = View::new(&self.client_world, &NoBodies, &self.fixture.tables);
        tick_player(
            view,
            Side::OwnClient,
            &start,
            &input,
            &mut self.client,
            &mut Vec::new(),
            &mut self.scratch,
        );
        let mut reported = Move {
            position: Some(self.client.body.position),
            look: None,
            on_ground: self.client.body.flags.on_ground,
            horizontal_collision: self.client.body.flags.horizontal_collision,
        };
        edit(&mut reported);
        let tick = ClientTick {
            input: self.input,
            reported,
            changes: states,
            sprint: self.sprint,
            flight: self.flight,
            item_use: self.item_use,
        };
        let view = View::new(&self.fixture.world, &NoBodies, &self.fixture.tables);
        self.prediction.tick(view, self.pushers.as_slice(), &tick)
    }
}

fn apply(change: &Change, state: &mut PlayerState) {
    match *change {
        Change::Motion(velocity) => state.body.velocity = velocity,
        Change::Impulse(impulse) => state.body.velocity += impulse,
        _ => {}
    }
}

fn passes(outcome: Outcome) -> bool {
    outcome == Outcome::default()
}

#[test]
fn pred_009_an_honest_client_that_walks_and_jumps_passes_each_tick() {
    let mut session = Session::new();
    let jump = Keys {
        jump: true,
        sprint: true,
        ..FORWARD
    };
    for tick in 0..40 {
        let keys = if tick % 10 < 5 { jump } else { FORWARD };
        assert!(passes(session.step(keys)), "tick {tick}");
    }
    assert!(
        session.prediction.accepted().body.position.x > 5.0,
        "the player walked"
    );
}

#[test]
fn rsp_001_a_report_far_above_the_prediction_sets_the_client_back_to_the_accepted_state() {
    let mut session = Session::new();
    session.step(FORWARD);
    let accepted = *session.prediction.accepted();
    let all = ChangeStates::default();
    let outcome = session.tick(FORWARD, all, 0, DVec3::ZERO, |reported| {
        if let Some(position) = &mut reported.position {
            position.y += 0.2;
        }
    });
    let setback = outcome.setback.expect("an offset of 0.2 sets back at once");
    assert_eq!(setback.position, accepted.body.position, "RSP-002");
    assert_eq!(
        setback.velocity, accepted.body.velocity,
        "RSP-001 keeps the velocity"
    );
    assert_eq!(setback.relative, Relative::YAW | Relative::PITCH);
    assert_eq!(session.step(FORWARD).mode, Mode::Exempt, "RSP-003");
    assert!(passes(session.prediction.accept(setback.position)));
}

#[test]
fn rsp_004_small_offsets_set_back_before_their_sum_reaches_one_block() {
    let mut session = Session::new();
    let mut sum = 0.0;
    let setback = loop {
        let outcome = session.tick(
            Keys::default(),
            ChangeStates::default(),
            0,
            DVec3::ZERO,
            |reported| {
                if let Some(position) = &mut reported.position {
                    position.x += 0.01;
                }
            },
        );
        if outcome.setback.is_some() {
            break outcome;
        }
        sum += outcome
            .offset
            .expect("each tick is ahead of the prediction");
        session.client = *session.prediction.accepted();
    };
    assert!(sum < 1.0, "MVC-INV-005: {sum}");
    assert!(setback.offset.is_some());
}

#[test]
fn pred_010_a_silent_tick_must_be_at_the_reference_position() {
    let mut session = Session::new();
    let silent = |reported: &mut Move| reported.position = None;
    let standing = session.tick(
        Keys::default(),
        ChangeStates::default(),
        0,
        DVec3::ZERO,
        silent,
    );
    assert!(passes(standing), "a client that stands still is silent");
    session.send(Change::Motion(DVec3::new(0.0, 0.4, 0.0)));
    let states = ChangeStates {
        applied: 1,
        seen: 1,
    };
    let hovering = session.tick(Keys::default(), states, 1, DVec3::ZERO, silent);
    assert!(
        hovering.offset.is_some(),
        "a client that moves up stays silent"
    );
}

#[test]
fn lat_004_a_change_in_its_window_passes_in_each_tick_the_client_can_apply_it() {
    for late in 0..3 {
        let mut session = Session::new();
        session.send(Change::Motion(DVec3::new(0.0, 0.42, 0.0)));
        let window = ChangeStates {
            applied: 0,
            seen: 1,
        };
        for tick in 0..3 {
            let client_applies = usize::from(tick >= late);
            let outcome = session.tick(FORWARD, window, client_applies, DVec3::ZERO, |_| {});
            assert!(passes(outcome), "applied in tick {late}, tick {tick}");
        }
        assert!(passes(session.step(FORWARD)), "applied in tick {late}");
    }
}

#[test]
fn lat_004_a_client_that_ignores_an_applied_change_fails() {
    let mut session = Session::new();
    session.send(Change::Motion(DVec3::new(0.0, 0.42, 0.0)));
    let applied = ChangeStates {
        applied: 1,
        seen: 1,
    };
    let outcome = session.tick(FORWARD, applied, 0, DVec3::ZERO, |_| {});
    assert!(outcome.offset.is_some(), "{outcome:?}");
}

/// A wall in front of the player goes away while the client has not applied the change: the
/// client walks into the old wall, which only the replay sees.
#[test]
fn pred_017_a_replay_sees_the_blocks_that_the_client_had() {
    let mut session = Session::new();
    let stone = session.fixture.blocks.stone;
    let wall = BlockPos::new(1, 0, 0);
    session
        .fixture
        .world
        .fill(wall, BlockPos::new(1, 1, 1), stone);
    session.client_world = session.fixture.world.clone();
    for y in 0..2 {
        for z in 0..2 {
            let at = BlockPos::new(1, y, z);
            session.send(Change::Block {
                at,
                old: stone,
                new: BlockStateId::AIR,
            });
        }
    }
    let window = ChangeStates {
        applied: 0,
        seen: 4,
    };
    for tick in 0..4 {
        let outcome = session.tick(FORWARD, window, 0, DVec3::ZERO, |_| {});
        assert!(passes(outcome), "tick {tick}: {outcome:?}");
    }
    assert!(
        session.client.body.flags.horizontal_collision,
        "the client walked into the old wall"
    );
    let applied = ChangeStates {
        applied: 4,
        seen: 4,
    };
    let outcome = session.tick(FORWARD, applied, 4, DVec3::ZERO, |_| {});
    assert!(passes(outcome), "{outcome:?}");
}

#[test]
fn pred_017_a_client_that_walks_through_a_wall_it_has_fails() {
    let mut session = Session::new();
    let stone = session.fixture.blocks.stone;
    session
        .fixture
        .world
        .fill(BlockPos::new(2, 0, 0), BlockPos::new(2, 1, 1), stone);
    let failed = (0..20).find_map(|_| session.step(FORWARD).offset);
    assert!(failed.is_some(), "the client walked through the wall");
}

#[test]
fn lat_008_an_accept_matches_the_destination_of_a_candidate() {
    let mut session = Session::new();
    let teleport = Teleport {
        position: DVec3::new(3.0, 0.0, 0.0),
        velocity: DVec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        relative: Relative::X | Relative::Y | Relative::Z,
    };
    let mut destination = session.client;
    let mut rotation = session.input.rotation;
    apply_teleport(&mut destination, &mut rotation, &teleport);

    session.prediction.teleport(teleport);
    let far = destination.body.position + DVec3::new(0.0, 0.0, 5.0);
    let outcome = session.prediction.accept(far);
    let correction = outcome
        .setback
        .expect("an accept elsewhere gets a correction");
    assert_eq!(correction.position, destination.body.position);
    assert_eq!(outcome.offset, Some(5.0));

    let outcome = session.prediction.accept(destination.body.position);
    assert!(passes(outcome));
    assert_eq!(
        session.prediction.accepted().body.position,
        destination.body.position
    );
    session.client = destination;
    assert!(passes(session.step(FORWARD)));
}

/// A mob on the side of `-x` pushes the player at each of its positions.
fn mob_at_the_side(session: &Session, jitter: f64) -> Pusher {
    let player = session.prediction.accepted().body.position;
    let at = player - DVec3::new(0.45, 0.0, 0.0);
    Pusher {
        low: at - DVec3::splat(jitter),
        high: at + DVec3::splat(jitter),
        half_width: 0.3,
        height: 1.95,
        complete: true,
    }
}

/// The push of the mob at the middle of its hull on the client.
fn push_of(pusher: &Pusher, player: DVec3) -> DVec3 {
    let at = (pusher.low + pusher.high) / 2.0;
    let mob = TestFixture::mob(0, at.x, at.z);
    let own = TestFixture::mob(1, player.x, player.z);
    -push_vector(&mob, &own).unwrap_or_default()
}

#[test]
fn unc_005_a_client_that_takes_the_push_passes_and_one_that_ignores_it_fails() {
    let mut session = Session::new();
    let pusher = mob_at_the_side(&session, 0.05);
    session.pushers = vec![pusher];
    let push = push_of(&pusher, session.client.body.position);
    let honest = session.tick(Keys::default(), ChangeStates::default(), 0, push, |_| {});
    assert!(passes(honest), "{honest:?}");

    let mut session = Session::new();
    session.pushers = vec![mob_at_the_side(&session, 0.05)];
    let ignored = session.tick(
        Keys::default(),
        ChangeStates::default(),
        0,
        DVec3::ZERO,
        |_| {},
    );
    assert!(ignored.offset.is_some(), "MVC-CHK-025: {ignored:?}");
}

#[test]
fn pred_019_too_many_pushers_make_the_tick_coarse_and_the_next_position_resyncs() {
    let mut session = Session::new();
    let pusher = mob_at_the_side(&session, 0.0);
    session.pushers = vec![pusher; 25];
    let push = push_of(&pusher, session.client.body.position) * 25.0;
    let coarse = session.tick(FORWARD, ChangeStates::default(), 0, push, |_| {});
    assert_eq!(coarse.mode, Mode::Coarse);
    session.pushers.clear();
    assert_eq!(session.step(FORWARD).mode, Mode::Resync);
    for tick in 0..10 {
        assert!(passes(session.step(FORWARD)), "tick {tick}");
    }
}

/// The client breaks the carpet `at` and walks two ticks. The server refuses: it sends the
/// carpet, which the client keeps, and one tick passes with the carpet applied in the order.
fn refuse_break(session: &mut Session, at: BlockPos) {
    let carpet = session.fixture.blocks.carpet;
    session.client_world.set(at, BlockStateId::AIR);
    session.step(FORWARD);
    session.step(FORWARD);
    session.send(Change::Block {
        at,
        old: carpet,
        new: carpet,
    });
    session.client_applied = 1;
    let kept = ChangeStates {
        applied: 1,
        seen: 1,
    };
    let outcome = session.tick(FORWARD, kept, 1, DVec3::ZERO, |_| {});
    assert!(passes(outcome), "the client keeps the carpet: {outcome:?}");
}

/// The `block_changed_ack` arrives in a window. The client applies the kept carpet `at` and
/// moves to `back` before its tick.
fn ack_and_move(session: &mut Session, at: BlockPos, back: DVec3) -> Outcome {
    session.send(Change::BlockAck);
    session.client_world.set(at, session.fixture.blocks.carpet);
    session.client_applied = 2;
    move_to(session, back, 2)
}

fn move_to(session: &mut Session, back: DVec3, seen: u64) -> Outcome {
    session.client.body.position = back;
    let window = ChangeStates { applied: 1, seen };
    session.tick(FORWARD, window, 2, DVec3::ZERO, |_| {})
}

/// A player on a carpet at the origin cell, which also holds a carpet far away.
fn on_carpets() -> (Session, BlockPos, BlockPos) {
    let mut session = Session::new();
    let carpet = session.fixture.blocks.carpet;
    let (under, far) = (BlockPos::new(0, 0, 0), BlockPos::new(10, 0, 10));
    for at in [under, far] {
        session.fixture.world.set(at, carpet);
        session.client_world.set(at, carpet);
    }
    (session, under, far)
}

/// The client breaks the carpet under it two times, one tick apart, and the server refuses.
/// Gives the positions of the two requests.
fn broken_two_times() -> (Session, BlockPos, [DVec3; 2]) {
    let (mut session, under, _) = on_carpets();
    session
        .prediction
        .block_requested(under, Some(BlockStateId::AIR));
    let first = session.client.body.position;
    session.step(FORWARD);
    session
        .prediction
        .block_requested(under, Some(BlockStateId::AIR));
    let second = session.client.body.position;
    refuse_break(&mut session, under);
    (session, under, [first, second])
}

#[test]
fn lat_007_the_client_moves_back_once_to_its_first_request_when_the_kept_state_meets_its_box() {
    let (mut session, under, [first, _]) = broken_two_times();
    let outcome = ack_and_move(&mut session, under, first);
    assert!(passes(outcome), "{outcome:?}");
    let again = move_to(&mut session, first, 2);
    assert!(
        again.offset.is_some(),
        "a run that applied the ack before does not move: {again:?}"
    );
}

#[test]
fn lat_007_a_later_request_keeps_the_first_position() {
    let (mut session, under, [_, second]) = broken_two_times();
    let outcome = ack_and_move(&mut session, under, second);
    assert!(outcome.offset.is_some(), "{outcome:?}");
}

#[test]
fn lat_007_a_client_does_not_move_back_before_the_ack_is_in_its_prefix() {
    let (mut session, _, [first, _]) = broken_two_times();
    session.send(Change::BlockAck);
    let outcome = move_to(&mut session, first, 1);
    assert!(outcome.offset.is_some(), "{outcome:?}");
}

#[test]
fn lat_007_a_client_does_not_move_back_when_no_kept_state_meets_its_box() {
    let (mut session, _, far) = on_carpets();
    session
        .prediction
        .block_requested(far, Some(BlockStateId::AIR));
    let first = session.client.body.position;
    refuse_break(&mut session, far);
    let outcome = ack_and_move(&mut session, far, first);
    assert!(outcome.offset.is_some(), "{outcome:?}");
}

#[test]
fn lat_007_a_request_after_the_ack_waits_for_a_later_ack() {
    let (mut session, under, far) = on_carpets();
    session
        .prediction
        .block_requested(far, Some(BlockStateId::AIR));
    session.send(Change::BlockAck);
    session
        .prediction
        .block_requested(under, Some(BlockStateId::AIR));
    let second = session.client.body.position;
    session.tick(FORWARD, ChangeStates::default(), 0, DVec3::ZERO, |_| {});
    session.client.body.position = second;
    let window = ChangeStates {
        applied: 0,
        seen: 1,
    };
    let outcome = session.tick(FORWARD, window, 1, DVec3::ZERO, |_| {});
    assert!(outcome.offset.is_some(), "{outcome:?}");
}

/// A player in front of a wall of stone two blocks high at `x = 1`.
fn before_a_wall() -> (Session, [BlockPos; 2]) {
    let mut session = Session::new();
    let stone = session.fixture.blocks.stone;
    let wall = [BlockPos::new(1, 0, 0), BlockPos::new(1, 1, 0)];
    for at in wall {
        session.fixture.world.set(at, stone);
        session.client_world.set(at, stone);
    }
    (session, wall)
}

/// The client breaks the wall. The server refuses: it sends the stone of each block, which the
/// client keeps, and then the `block_changed_ack`, which has the number 2.
fn refused_wall_break() -> Session {
    let (mut session, wall) = before_a_wall();
    let stone = session.fixture.blocks.stone;
    for at in wall {
        session
            .prediction
            .block_requested(at, Some(BlockStateId::AIR));
        session.client_world.set(at, BlockStateId::AIR);
        session.send(Change::Block {
            at,
            old: stone,
            new: stone,
        });
    }
    session.send(Change::BlockAck);
    session.client_applied = 3;
    session
}

/// Walks 15 honest ticks with these states of the changes; gives the `x` of the player.
fn walk(session: &mut Session, states: ChangeStates) -> f64 {
    for tick in 0..15 {
        let outcome = session.tick(FORWARD, states, 3, DVec3::ZERO, |_| {});
        assert!(passes(outcome), "tick {tick}: {outcome:?}");
    }
    session.prediction.accepted().body.position.x
}

#[test]
fn lat_007_until_the_ack_the_client_has_its_edit_and_not_the_update() {
    let mut session = refused_wall_break();
    let before_ack = ChangeStates {
        applied: 2,
        seen: 2,
    };
    let x = walk(&mut session, before_ack);
    assert!(x > 1.5, "the client walked into the cells it broke: {x}");
}

#[test]
fn lat_007_the_kept_state_enters_at_the_ack() {
    let mut session = refused_wall_break();
    let stone = session.fixture.blocks.stone;
    for y in 0..2 {
        session.client_world.set(BlockPos::new(1, y, 0), stone);
    }
    let ack_in_window = ChangeStates {
        applied: 2,
        seen: 3,
    };
    let x = walk(&mut session, ack_in_window);
    assert!(x < 1.0, "the kept stone stops the client: {x}");
}

#[test]
fn lat_007_an_unsettled_edit_makes_a_tick_coarse_when_the_grown_box_touches_it() {
    let mut session = Session::new();
    session
        .prediction
        .block_requested(BlockPos::new(1, 0, 1), None);
    let near = session.step(Keys::default());
    assert!(
        passes(near),
        "a still player does not touch the block beside it: {near:?}"
    );
    session
        .prediction
        .block_requested(BlockPos::new(0, -1, 0), None);
    let under = session.step(Keys::default());
    assert_eq!(
        under.mode,
        Mode::Coarse,
        "the player stands on the block: {under:?}"
    );
}

#[test]
fn pred_016_too_many_open_windows_at_the_step_cap_set_back() {
    let mut session = Session::new();
    for _ in 0..70 {
        session.send(Change::Impulse(DVec3::new(0.0, 0.01, 0.0)));
    }
    let window = ChangeStates {
        applied: 0,
        seen: 70,
    };
    let outcome = session.tick(FORWARD, window, 0, DVec3::ZERO, |reported| {
        if let Some(position) = &mut reported.position {
            position.y += 3.0;
        }
    });
    assert!(outcome.setback.is_some(), "{outcome:?}");
}

#[test]
fn pred_011_wrong_flags_are_flagged_and_do_not_select_candidates() {
    let mut session = Session::new();
    let all = ChangeStates::default();
    let ground = session.tick(FORWARD, all, 0, DVec3::ZERO, |reported| {
        reported.on_ground = !reported.on_ground;
    });
    assert!(ground.wrong_on_ground, "{ground:?}");
    assert_eq!((ground.offset, ground.setback), (None, None), "{ground:?}");
    let wall = session.tick(FORWARD, all, 0, DVec3::ZERO, |reported| {
        reported.horizontal_collision = !reported.horizontal_collision;
    });
    assert!(wall.wrong_horizontal_collision, "{wall:?}");
    assert!(!wall.wrong_on_ground, "{wall:?}");
    assert!(passes(session.step(FORWARD)));
}

#[test]
fn lat_005_an_impulse_adds_to_the_velocity_of_the_candidate() {
    let mut session = Session::new();
    session.step(FORWARD);
    session.send(Change::Impulse(DVec3::new(0.1, 0.3, 0.0)));
    for tick in 0..5 {
        assert!(passes(session.step(FORWARD)), "tick {tick}");
    }
}

#[test]
fn pred_006_each_value_of_a_sprint_a_flight_and_an_item_use_change_passes() {
    for combination in 0..8_u8 {
        let [sprints, flies, uses] = [0, 1, 2].map(|bit| combination >> bit & 1 == 1);
        let mut session = Session::new();
        session.sprint = Some(true);
        session.flight = Some(true);
        session.item_use = Some(true);
        session.client.sprint0 = sprints;
        session.client.flight0 = flies;
        session.client.item_use.using = uses;
        let outcome = session.tick(FORWARD, ChangeStates::default(), 0, DVec3::ZERO, |_| {});
        assert!(
            passes(outcome),
            "sprint {sprints}, flight {flies}, item use {uses}: {outcome:?}"
        );
    }
}

#[test]
fn pred_008_the_offset_of_a_tick_is_the_smallest_offset_of_its_candidates() {
    let mut session = Session::new();
    session.sprint = Some(true);
    session.client.sprint0 = false;
    let outcome = session.tick(
        FORWARD,
        ChangeStates::default(),
        0,
        DVec3::ZERO,
        |reported| {
            if let Some(position) = &mut reported.position {
                position.y += 0.05;
            }
        },
    );
    let offset = outcome
        .offset
        .expect("no candidate is 0.05 above the prediction");
    assert!(
        (offset - 0.05).abs() < 1e-9,
        "the candidate without the sprint is only 0.05 away: {offset}"
    );
}

#[test]
fn rsp_004_the_buffer_decays_in_each_tick_without_a_violation() {
    let mut session = Session::new();
    let ahead = |reported: &mut Move| {
        if let Some(position) = &mut reported.position {
            position.x += 0.09;
        }
    };
    for violation in 0..11 {
        let outcome = session.tick(
            Keys::default(),
            ChangeStates::default(),
            0,
            DVec3::ZERO,
            ahead,
        );
        assert!(
            outcome.setback.is_none(),
            "violation {violation}: the buffer is below 1"
        );
        session.client = *session.prediction.accepted();
    }
    for _ in 0..1000 {
        session.step(Keys::default());
    }
    let outcome = session.tick(
        Keys::default(),
        ChangeStates::default(),
        0,
        DVec3::ZERO,
        ahead,
    );
    assert!(
        outcome.offset.is_some() && outcome.setback.is_none(),
        "1000 ticks shrink the buffer of 0.99 by the factor 0.999^1000: {outcome:?}"
    );
}
