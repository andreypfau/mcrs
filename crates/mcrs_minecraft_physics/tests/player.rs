//! Kernel tests of the tick of a player, its server copy, the observed velocity and the teleport.

use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::BlockAction;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::KeyedStream;
use mcrs_minecraft_physics::Keys;
use mcrs_minecraft_physics::MoveFlags;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::Observation;
use mcrs_minecraft_physics::PlayerInput;
use mcrs_minecraft_physics::PlayerRotation;
use mcrs_minecraft_physics::PlayerState;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::PushTargets;
use mcrs_minecraft_physics::Relative;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::ServerCopy;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::SourceKind;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TeamSlot;
use mcrs_minecraft_physics::Teleport;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::apply_teleport;
use mcrs_minecraft_physics::observed_velocity;
use mcrs_minecraft_physics::server_part;
use mcrs_minecraft_physics::tick_player;
use mcrs_voxel::BlockPos;

struct Fixture {
    blocks: TestBlocks,
    tables: Tables,
    world: TestWorld,
    scratch: Scratch,
    actions: Vec<BlockAction>,
}

impl Fixture {
    fn on_stone() -> Self {
        let blocks = TestBlocks::new();
        let tables = Tables::new(blocks.table.clone());
        let mut world = TestWorld::new();
        world.fill(
            BlockPos::new(-32, 0, -32),
            BlockPos::new(32, 0, 32),
            blocks.stone,
        );
        Self {
            blocks,
            tables,
            world,
            scratch: Scratch::new(),
            actions: Vec::new(),
        }
    }

    fn view(&self) -> View<'_, TestWorld, NoBodies> {
        View {
            world: &self.world,
            bodies: &NoBodies,
            tables: &self.tables,
        }
    }

    fn tick(&mut self, state: &PlayerState, keys: Keys, yaw: f32) -> PlayerState {
        let input = PlayerInput {
            keys,
            rotation: PlayerRotation { yaw, pitch: 0.0 },
            ..PlayerInput::default()
        };
        self.tick_with(state, &input)
    }

    fn tick_with(&mut self, state: &PlayerState, input: &PlayerInput) -> PlayerState {
        let mut out = PlayerState::default();
        let view = View {
            world: &self.world,
            bodies: &NoBodies,
            tables: &self.tables,
        };
        self.actions.clear();
        tick_player(
            view,
            Side::OwnClient,
            state,
            input,
            &mut out,
            &mut self.actions,
            &mut self.scratch,
        );
        out
    }

    fn settled(&mut self) -> PlayerState {
        let mut state = PlayerState::new(DVec3::new(0.5, 1.0, 0.5));
        for _ in 0..3 {
            state = self.tick(&state, Keys::default(), 0.0);
        }
        state
    }
}

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

#[test]
fn a_standing_player_rests_on_the_ground() {
    let mut fixture = Fixture::on_stone();
    let state = fixture.settled();
    assert!(state.flags.on_ground);
    assert_eq!(state.velocity.y, -0.0784000015258789);
    let next = fixture.tick(&state, Keys::default(), 0.0);
    assert_eq!(next, state);
}

#[test]
fn inp_001_a_diagonal_input_is_as_long_as_one_or_longer() {
    let mut fixture = Fixture::on_stone();
    let state = fixture.settled();
    let straight = fixture.tick(&state, FORWARD, 0.0).velocity;
    let diagonal = fixture
        .tick(
            &state,
            Keys {
                left: true,
                ..FORWARD
            },
            0.0,
        )
        .velocity;
    let straight_speed = straight.x.hypot(straight.z);
    let diagonal_speed = diagonal.x.hypot(diagonal.z);
    // The straight input has the length 0.98F; the diagonal one reaches the limit of 1.
    assert!((diagonal_speed / straight_speed - 1.0 / f64::from(0.98_f32)).abs() < 1.0e-6);
}

#[test]
fn inp_017_a_second_tap_of_forward_in_the_window_starts_the_sprint() {
    // The timer of 7 counts down at the start of each tick, the press tick too: 5 ticks between
    // the taps is the longest gap.
    for (gap, sprints) in [(3, true), (5, true), (6, false)] {
        let mut fixture = Fixture::on_stone();
        let mut state = fixture.settled();
        state = fixture.tick(&state, FORWARD, 0.0);
        for _ in 0..gap {
            state = fixture.tick(&state, Keys::default(), 0.0);
        }
        state = fixture.tick(&state, FORWARD, 0.0);
        assert_eq!(state.sprint0, sprints, "a gap of {gap} ticks");
    }
}

#[test]
fn inp_018_two_presses_of_jump_toggle_the_flight_state() {
    for (gap, flies) in [(2, true), (6, true), (7, false)] {
        let mut fixture = Fixture::on_stone();
        let mut state = fixture.settled();
        state.may_fly = true;
        let press = Keys {
            jump: true,
            ..Keys::default()
        };
        state = fixture.tick(&state, press, 0.0);
        for _ in 1..gap {
            state = fixture.tick(&state, Keys::default(), 0.0);
        }
        let toggled = fixture.tick(&state, press, 0.0);
        assert_eq!(toggled.flight0, flies, "a gap of {gap} ticks");
    }
}

#[test]
fn inp_018_a_held_or_automatic_jump_key_does_not_toggle_the_flight_state() {
    let held = Keys {
        jump: true,
        ..Keys::default()
    };
    let automatic = Keys {
        auto_jump: true,
        ..held
    };
    for (jump0, keys) in [(true, held), (false, automatic)] {
        let mut fixture = Fixture::on_stone();
        let mut state = fixture.settled();
        state.may_fly = true;
        state.jump_timer = 3;
        state.jump0 = jump0;
        let next = fixture.tick(&state, keys, 0.0);
        assert!(!next.flight0, "jump0 {jump0}, auto-jump {}", keys.auto_jump);
        assert!(next.jump0);
    }
}

#[test]
fn inp_015_flight_that_starts_on_the_ground_jumps_before_the_impulse() {
    let mut fixture = Fixture::on_stone();
    let mut state = fixture.settled();
    state.may_fly = true;
    state.jump_timer = 3;
    let press = Keys {
        jump: true,
        ..Keys::default()
    };
    let next = fixture.tick(&state, press, 0.0);
    assert!(next.flight0);
    let rise = f64::from(0.42_f32) + f64::from(0.05_f32 * 3.0);
    assert_eq!(next.position.y - state.position.y, rise);
}

#[test]
fn psh_008_the_push_enters_the_velocity_before_the_zero_cutoff() {
    let mut fixture = Fixture::on_stone();
    let state = fixture.settled();
    let input = PlayerInput {
        push: DVec3::new(0.002, 0.0, 0.0),
        ..PlayerInput::default()
    };
    let next = fixture.tick_with(&state, &input);
    assert_eq!(next.position.x, state.position.x);
}

#[test]
fn inp_012_a_sneaking_player_stays_on_the_edge() {
    let mut fixture = Fixture::on_stone();
    fixture.world = TestWorld::new();
    fixture
        .world
        .set(BlockPos::new(0, 0, 0), fixture.blocks.stone);
    let state = fixture.settled();
    let sneak = Keys {
        shift: true,
        ..FORWARD
    };
    for yaw in [0.0, 90.0, 180.0, 270.0, 45.0] {
        let mut walker = state;
        for _ in 0..40 {
            walker = fixture.tick(&walker, sneak, yaw);
            assert!(walker.position.y >= 1.0, "fell off the edge at yaw {yaw}");
        }
        assert!(walker.flags.on_ground);
    }
}

#[test]
fn srv_008_the_observed_velocity_holds_the_velocity_of_the_tick() {
    let mut fixture = Fixture::on_stone();
    fixture.world.fill(
        BlockPos::new(3, 1, -32),
        BlockPos::new(3, 2, 32),
        fixture.blocks.stone,
    );
    let mut state = fixture.settled();
    let walk = Keys {
        right: true,
        ..FORWARD
    };
    for tick in 0..60 {
        let yaw = (tick as f32) * 13.0 - 100.0;
        let next = fixture.tick(&state, walk, yaw);
        let observation = Observation {
            displacement: next.position - state.position,
            end_flags: next.flags,
            end_support: next.support,
            flight: false,
            jump: false,
            shift: false,
        };
        let candidates = observed_velocity(fixture.view(), &state, &observation);
        let expected = if next.flags.horizontal_collision {
            3
        } else {
            1
        };
        assert_eq!(candidates.as_slice().len(), expected);
        // The displacement from the positions can differ from the resolved move by one unit in
        // the last place, so the candidate can too.
        let near = |candidate: &DVec3| (*candidate - next.velocity).abs().max_element() <= 1.0e-15;
        assert!(
            candidates.as_slice().iter().any(near),
            "tick {tick}: {:?} not in {:?}",
            next.velocity,
            candidates.as_slice()
        );
        state = next;
    }
}

#[test]
fn srv_008_slow_falling_reads_the_vertical_velocity_before_the_climb() {
    let mut fixture = Fixture::on_stone();
    fixture
        .world
        .set(BlockPos::new(0, 5, 0), fixture.blocks.scaffolding);
    let mut state = PlayerState::new(DVec3::new(0.5, 5.2, 0.5));
    state.effects.slow_falling = true;
    let observation = Observation {
        displacement: DVec3::new(0.0, -0.05, 0.0),
        end_flags: MoveFlags::default(),
        end_support: None,
        flight: false,
        jump: true,
        shift: false,
    };
    let candidates = observed_velocity(fixture.view(), &state, &observation);
    let [velocity] = candidates.as_slice() else {
        panic!("one candidate without a horizontal collision");
    };
    assert_eq!(velocity.y, (0.2 - 0.01) * f64::from(0.98_f32));
}

#[test]
fn srv_010_a_teleport_rotates_the_velocity_by_the_table() {
    let tables = Tables::new(TestBlocks::new().table);
    let mut state = PlayerState::new(DVec3::new(10.0, 64.0, -3.0));
    state.velocity = DVec3::new(1.0, 0.5, 0.0);
    state.flags = MoveFlags {
        on_ground: true,
        ..MoveFlags::default()
    };
    state.fall_distance = 2.0;
    let mut rotation = PlayerRotation {
        yaw: 0.0,
        pitch: 0.0,
    };
    let packet = Teleport {
        position: DVec3::new(1.0, 2.0, 3.0),
        velocity: DVec3::new(0.0, 0.25, 0.0),
        yaw: 90.0,
        pitch: 100.0,
        relative: Relative::X | Relative::ROTATE_VELOCITY | Relative::VELOCITY_Y,
    };
    apply_teleport(&tables.sine, &mut state, &mut rotation, &packet);
    assert_eq!(state.position, DVec3::new(11.0, 2.0, 3.0));
    assert_eq!((rotation.yaw, rotation.pitch), (90.0, 90.0));
    // The pitch turns by -90 degrees and the yaw by -90 degrees, both exact in the table.
    assert_eq!(state.velocity, DVec3::new(0.0, 0.25, 0.0));
    assert!(state.flags.on_ground && state.fall_distance == 2.0);
}

#[test]
fn cmp_023_the_server_copy_decays_before_the_cutoff() {
    let mut fixture = Fixture::on_stone();
    let mut state = fixture.settled();
    let row = GridRow {
        position: state.position,
        half_width: 0.3,
        height: 1.8,
        key: BirthKey::new(0, SourceKind::Player, 0, 0),
        id: EntityId(1),
        root_vehicle: EntityId(1),
        team: TeamSlot::default(),
        flags: GridFlags::PUSHABLE | GridFlags::PLAYER,
        push_targets: PushTargets::Each,
    };
    let copy = ServerCopy {
        start: state.position,
        row: &row,
        rules: PushRules::default(),
        stream: KeyedStream::new(&row.key, 1),
    };
    // 0.003 survives the cutoff of 9.0E-6 on the squared length; 0.98 × 0.003 does not.
    state.velocity = DVec3::new(0.003, 0.0, 0.0);
    let view = fixture.view();
    server_part(
        view,
        &mut state,
        &copy,
        &mut Vec::new(),
        &mut Scratch::new(),
    );
    assert_eq!(state.velocity.x, 0.0);
    state.velocity = DVec3::new(0.1, 0.0, 0.0);
    server_part(
        view,
        &mut state,
        &copy,
        &mut Vec::new(),
        &mut Scratch::new(),
    );
    assert_eq!(state.velocity.x, 0.1 * 0.98);
}

#[test]
#[ignore = "the vanilla client recorder (T1) does not exist yet; E9 to E14 and E21 to E23 need it"]
fn own_player_follows_the_recorded_client_ticks() {
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/player");
    assert!(
        std::path::Path::new(directory).is_dir(),
        "no recorded client ticks in {directory}"
    );
}
