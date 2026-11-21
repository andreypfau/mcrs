#[derive(Clone, Debug)]
pub struct Placement {
    pub at: (i32, i32, i32),
    pub state: &'static str,
}

#[derive(Clone, Debug)]
pub struct Probe {
    pub at: (i32, i32, i32),
    pub candidates: Vec<&'static str>,
}

#[derive(Clone, Debug)]
pub struct Scenario {
    pub name: &'static str,
    pub build: Vec<Placement>,
    pub actions: Vec<(usize, Placement)>,
    pub probes: Vec<Probe>,
    pub ticks: usize,
}

fn at(x: i32, y: i32, z: i32, state: &'static str) -> Placement {
    Placement {
        at: (x, y, z),
        state,
    }
}

fn probe(x: i32, y: i32, z: i32, candidates: &[&'static str]) -> Probe {
    Probe {
        at: (x, y, z),
        candidates: candidates.to_vec(),
    }
}

const LAMP: [&str; 2] = [
    "minecraft:redstone_lamp[lit=false]",
    "minecraft:redstone_lamp[lit=true]",
];
const PISTON_EAST: [&str; 2] = [
    "minecraft:piston[facing=east,extended=false]",
    "minecraft:piston[facing=east,extended=true]",
];
const STICKY_EAST: [&str; 2] = [
    "minecraft:sticky_piston[facing=east,extended=false]",
    "minecraft:sticky_piston[facing=east,extended=true]",
];
const TORCH: [&str; 2] = [
    "minecraft:redstone_torch[lit=false]",
    "minecraft:redstone_torch[lit=true]",
];
const STONE_OR_AIR: [&str; 2] = ["minecraft:stone", "minecraft:air"];

fn wire_powers() -> Vec<&'static str> {
    const POWERS: [&str; 16] = [
        "minecraft:redstone_wire[power=0]",
        "minecraft:redstone_wire[power=1]",
        "minecraft:redstone_wire[power=2]",
        "minecraft:redstone_wire[power=3]",
        "minecraft:redstone_wire[power=4]",
        "minecraft:redstone_wire[power=5]",
        "minecraft:redstone_wire[power=6]",
        "minecraft:redstone_wire[power=7]",
        "minecraft:redstone_wire[power=8]",
        "minecraft:redstone_wire[power=9]",
        "minecraft:redstone_wire[power=10]",
        "minecraft:redstone_wire[power=11]",
        "minecraft:redstone_wire[power=12]",
        "minecraft:redstone_wire[power=13]",
        "minecraft:redstone_wire[power=14]",
        "minecraft:redstone_wire[power=15]",
    ];
    POWERS.to_vec()
}

#[expect(
    clippy::too_many_lines,
    reason = "the corpus lists each circuit with its blocks and probes in one place"
)]
pub fn corpus() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "quasi connectivity waits for an update",
            build: vec![
                at(0, 0, 0, "minecraft:piston[facing=east]"),
                at(0, 1, 1, "minecraft:redstone_block"),
            ],
            actions: vec![(4, at(0, 0, 1, "minecraft:stone"))],
            probes: vec![probe(0, 0, 0, &PISTON_EAST)],
            ticks: 10,
        },
        Scenario {
            name: "observer clock",
            build: vec![
                at(0, 0, 0, "minecraft:observer[facing=east]"),
                at(1, 0, 0, "minecraft:observer[facing=west]"),
            ],
            actions: vec![(2, at(0, 1, 0, "minecraft:stone"))],
            probes: vec![
                probe(
                    0,
                    0,
                    0,
                    &[
                        "minecraft:observer[facing=east,powered=false]",
                        "minecraft:observer[facing=east,powered=true]",
                    ],
                ),
                probe(
                    1,
                    0,
                    0,
                    &[
                        "minecraft:observer[facing=west,powered=false]",
                        "minecraft:observer[facing=west,powered=true]",
                    ],
                ),
            ],
            ticks: 14,
        },
        Scenario {
            name: "comparator subtracts its side",
            build: vec![
                at(0, 0, 0, "minecraft:redstone_block"),
                at(1, 0, 0, "minecraft:comparator[facing=west,mode=subtract]"),
                at(2, 0, 0, "minecraft:redstone_wire"),
                at(1, 0, 1, "minecraft:redstone_wire"),
                at(1, 0, 2, "minecraft:redstone_wire"),
                at(1, 0, 3, "minecraft:redstone_wire"),
                at(1, 0, 4, "minecraft:redstone_wire"),
                at(1, 0, 5, "minecraft:redstone_wire"),
                at(1, 0, 6, "minecraft:redstone_wire"),
            ],
            actions: vec![(2, at(1, 0, 7, "minecraft:redstone_block"))],
            probes: vec![probe(2, 0, 0, &wire_powers())],
            ticks: 10,
        },
        Scenario {
            name: "torch clock burns out",
            build: vec![
                at(0, 0, 0, "minecraft:stone"),
                at(1, 0, 0, "minecraft:redstone_wire"),
                at(2, 0, 0, "minecraft:redstone_wire"),
                at(2, 0, 1, "minecraft:redstone_wire"),
                at(2, 0, 2, "minecraft:redstone_wire"),
                at(1, 0, 2, "minecraft:redstone_wire"),
                at(0, 0, 2, "minecraft:redstone_wire"),
                at(0, 0, 1, "minecraft:redstone_wire"),
            ],
            actions: vec![(1, at(0, 1, 0, "minecraft:redstone_torch"))],
            probes: vec![probe(0, 1, 0, &TORCH)],
            ticks: 240,
        },
        Scenario {
            name: "twelve blocks push and thirteen do not",
            build: vec![
                at(0, 0, 0, "minecraft:piston[facing=east]"),
                at(1, 0, 0, "minecraft:stone"),
                at(2, 0, 0, "minecraft:stone"),
                at(3, 0, 0, "minecraft:stone"),
                at(4, 0, 0, "minecraft:stone"),
                at(5, 0, 0, "minecraft:stone"),
                at(6, 0, 0, "minecraft:stone"),
                at(7, 0, 0, "minecraft:stone"),
                at(8, 0, 0, "minecraft:stone"),
                at(9, 0, 0, "minecraft:stone"),
                at(10, 0, 0, "minecraft:stone"),
                at(11, 0, 0, "minecraft:stone"),
                at(12, 0, 0, "minecraft:stone"),
                at(0, 0, 4, "minecraft:piston[facing=east]"),
                at(1, 0, 4, "minecraft:stone"),
                at(2, 0, 4, "minecraft:stone"),
                at(3, 0, 4, "minecraft:stone"),
                at(4, 0, 4, "minecraft:stone"),
                at(5, 0, 4, "minecraft:stone"),
                at(6, 0, 4, "minecraft:stone"),
                at(7, 0, 4, "minecraft:stone"),
                at(8, 0, 4, "minecraft:stone"),
                at(9, 0, 4, "minecraft:stone"),
                at(10, 0, 4, "minecraft:stone"),
                at(11, 0, 4, "minecraft:stone"),
                at(12, 0, 4, "minecraft:stone"),
                at(13, 0, 4, "minecraft:stone"),
            ],
            actions: vec![
                (2, at(0, 1, 0, "minecraft:redstone_block")),
                (2, at(0, 1, 4, "minecraft:redstone_block")),
            ],
            probes: vec![
                probe(0, 0, 0, &PISTON_EAST),
                probe(13, 0, 0, &STONE_OR_AIR),
                probe(0, 0, 4, &PISTON_EAST),
                probe(14, 0, 4, &STONE_OR_AIR),
            ],
            ticks: 8,
        },
        Scenario {
            name: "a sticky piston on a one tick pulse leaves its block",
            build: vec![
                at(0, 0, 0, "minecraft:sticky_piston[facing=east]"),
                at(1, 0, 0, "minecraft:stone"),
                at(-1, 0, 0, "minecraft:observer[facing=west]"),
            ],
            actions: vec![(2, at(-2, 0, 0, "minecraft:stone"))],
            probes: vec![
                probe(0, 0, 0, &STICKY_EAST),
                probe(2, 0, 0, &STONE_OR_AIR),
                probe(1, 0, 0, &STONE_OR_AIR),
            ],
            ticks: 10,
        },
        Scenario {
            name: "two sticky pistons pull one block and the update order decides",
            build: vec![
                at(
                    0,
                    0,
                    0,
                    "minecraft:sticky_piston[facing=east,extended=true]",
                ),
                at(1, 0, 0, "minecraft:piston_head[facing=east,type=sticky]"),
                at(2, 0, 0, "minecraft:stone"),
                at(3, 0, 0, "minecraft:piston_head[facing=west,type=sticky]"),
                at(
                    4,
                    0,
                    0,
                    "minecraft:sticky_piston[facing=west,extended=true]",
                ),
                at(0, 1, 0, "minecraft:redstone_block"),
                at(4, 1, 0, "minecraft:redstone_block"),
            ],
            actions: vec![
                (2, at(0, 1, 0, "minecraft:air")),
                (2, at(4, 1, 0, "minecraft:air")),
            ],
            probes: vec![
                probe(1, 0, 0, &STONE_OR_AIR),
                probe(3, 0, 0, &STONE_OR_AIR),
                probe(2, 0, 0, &STONE_OR_AIR),
            ],
            ticks: 8,
        },
        Scenario {
            name: "a wire line lights a lamp fifteen blocks away",
            build: {
                let mut build = vec![at(-16, 0, 0, "minecraft:redstone_lamp")];
                for x in -15..=-1 {
                    build.push(at(x, 0, 0, "minecraft:redstone_wire"));
                }
                build.push(at(0, 0, 0, "minecraft:redstone_lamp"));
                for x in 1..=16 {
                    build.push(at(x, 0, 0, "minecraft:redstone_wire"));
                }
                build
            },
            actions: vec![(2, at(0, 1, 0, "minecraft:redstone_block"))],
            probes: vec![
                probe(-16, 0, 0, &LAMP),
                probe(-15, 0, 0, &wire_powers()),
                probe(-1, 0, 0, &wire_powers()),
                probe(16, 0, 0, &wire_powers()),
            ],
            ticks: 6,
        },
    ]
}
