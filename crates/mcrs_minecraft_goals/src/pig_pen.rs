use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::hash::Hash;
use core::hash::Hasher;

use glam::DVec3;
use mcrs_minecraft_brain::AiDriver;
use mcrs_minecraft_brain::Applied;
use mcrs_minecraft_brain::Cadence;
use mcrs_minecraft_brain::ControlFlags;
use mcrs_minecraft_brain::DamageCause;
use mcrs_minecraft_brain::DamageRecord;
use mcrs_minecraft_brain::Decision;
use mcrs_minecraft_brain::Fact;
use mcrs_minecraft_brain::FactKind;
use mcrs_minecraft_brain::KeyedRandom;
use mcrs_minecraft_brain::LookIntent;
use mcrs_minecraft_brain::MobFlags;
use mcrs_minecraft_brain::MobView;
use mcrs_minecraft_brain::Panic;
use mcrs_minecraft_brain::Part;
use mcrs_minecraft_brain::PathIntent;
use mcrs_minecraft_brain::PlayerFlags;
use mcrs_minecraft_brain::PlayerRow;
use mcrs_minecraft_brain::PlayerTable;
use mcrs_minecraft_brain::Progress;
use mcrs_minecraft_brain::Stage;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::TickContext;
use mcrs_minecraft_brain::Ticket;
use mcrs_minecraft_brain::TileWork;
use mcrs_minecraft_brain::WorldSeed;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_voxel::BlockPos;

use crate::FlatGround;
use crate::IDLE_RESET_RANGE;
use crate::PigBrain;
use crate::Pond;
use crate::STROLL_IDLE_LIMIT;

/// How the pen sets the idle time of a pig.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IdleRule {
    /// As the reference: 0 while a player is within 32 blocks, else one more in each tick.
    FromPlayers,
    /// Always 0, as if a player stood near each pig.
    AlwaysNear,
    /// Always above the limit of the stroll, as if each player were far.
    AlwaysFar,
}

/// The scene of a pen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenConfig {
    pub side: i32,
    pub pigs: u32,
    pub players: u32,
    /// The chance of a hit of a type that causes panic, for each pig in each tick.
    pub hurt_chance: f64,
    pub idle: IdleRule,
    pub seed: u64,
}

/// A host for tests and benches of the stage: pigs on a [`FlatGround`] with a few players that do
/// not move. In each tick it publishes the snapshot, runs the stage, applies the due decisions and
/// moves each pig straight to its target at the steady speed of a walk on grass.
#[derive(Debug)]
pub struct PigPen {
    config: PenConfig,
    ground: FlatGround,
    driver: AiDriver<PigBrain>,
    players: Vec<PlayerRow>,
    pigs: Vec<HostPig>,
    facts: BTreeMap<TileKey, Vec<Fact>>,
    moves: Vec<(EntityId, TileKey)>,
    decisions: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HostPig {
    key: EntityKey,
    position: DVec3,
    tile: TileKey,
    cadence: Cadence,
    walk: Option<Walk>,
    path: Applied,
    look: Look,
    idle_time: u32,
    damage: Option<DamageRecord>,
    panic: Panic,
    on_fire: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Walk {
    target: DVec3,
    per_tick: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    Idle,
    At { hold: u8 },
    Around,
}

/// What one tick of the pen did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PenTick {
    pub tick: u64,
    pub decisions: u64,
}

const HURT: Part = Part(0xF000);
const PLACE: Part = Part(0xF001);
const PIG_SPEED: f64 = 0.25;
const GRASS_WALK: f64 = 2.2026;
const FAR_IDLE: u32 = 1000;

impl PigPen {
    #[must_use]
    pub fn new(config: PenConfig, stage_config: StageConfig) -> Self {
        let seed = WorldSeed(config.seed);
        let side = f64::from(config.side);
        let mut driver = AiDriver::new(stage_config);
        let pigs: Vec<HostPig> = (0..config.pigs)
            .map(|index| {
                let key = EntityKey(u128::from(index) << 64);
                let mut random = KeyedRandom::new(seed, &key, 0, PLACE);
                let position = DVec3::new(random.unit() * side, 0.0, random.unit() * side);
                let offset = u8::try_from(random.below(4)).unwrap_or(0);
                HostPig {
                    key,
                    position,
                    tile: TileKey::at(position),
                    cadence: Cadence::new(1, offset),
                    walk: None,
                    path: Applied::default(),
                    look: Look::Idle,
                    idle_time: match config.idle {
                        IdleRule::AlwaysFar => FAR_IDLE,
                        IdleRule::AlwaysNear | IdleRule::FromPlayers => 0,
                    },
                    damage: None,
                    panic: Panic::Calm,
                    on_fire: false,
                }
            })
            .collect();
        for (index, pig) in pigs.iter().enumerate() {
            driver.spawn(
                pig.tile,
                mob(index),
                pig.key,
                pig.cadence,
                PigBrain::default(),
            );
        }
        let players = (0..config.players)
            .map(|index| {
                let key = EntityKey(u128::from(index) << 64 | 1);
                let mut random = KeyedRandom::new(seed, &key, 0, PLACE);
                PlayerRow {
                    player: EntityId(u64::from(index)),
                    key,
                    position: DVec3::new(random.unit() * side, 0.0, random.unit() * side),
                    eye_height: 1.62,
                    flags: PlayerFlags::ALIVE,
                    visibility: 1.0,
                    vehicle: None,
                }
            })
            .collect();
        Self {
            config,
            ground: FlatGround {
                side: config.side,
                pond: None,
            },
            driver,
            players,
            pigs,
            facts: BTreeMap::new(),
            moves: Vec::new(),
            decisions: 0,
        }
    }

    /// One tick: the snapshot, the stage through `run` (a parallel map over the tile tasks, or
    /// [`run_tiles`]), the application of the due decisions and the hot step of each pig.
    pub fn step(
        &mut self,
        run: impl FnOnce(&mut [TileWork<'_, PigBrain>], &TickContext<'_, FlatGround>),
    ) -> PenTick {
        let tick = self.driver.begin_tick();
        self.publish();
        let decisions = self.driver.run_stage(&self.ground, run) as u64;
        self.decisions += decisions;
        for issued in self.driver.take_due() {
            for decision in issued.tiles.values().flatten() {
                self.apply(decision);
            }
        }
        self.hot_step(tick);
        PenTick { tick, decisions }
    }

    /// The tick of the next [`PigPen::step`].
    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.driver.tick().saturating_add(1)
    }

    #[must_use]
    pub const fn stage(&self) -> &Stage<PigBrain> {
        self.driver.stage()
    }

    #[must_use]
    pub const fn decisions(&self) -> u64 {
        self.decisions
    }

    /// The ticks in which a due decision was not ready; 0 when the stage runs inside the tick.
    #[must_use]
    pub const fn waits(&self) -> u64 {
        self.driver.waits()
    }

    /// The count of pigs that walk now.
    #[must_use]
    pub fn walking(&self) -> usize {
        self.pigs.iter().filter(|pig| pig.walk.is_some()).count()
    }

    /// A hash of the pigs and of every brain: equal runs give equal hashes in each tick.
    #[must_use]
    pub fn state_hash(&self, scope: HashScope) -> u64 {
        let mut hasher = StateHasher::default();
        for pig in &self.pigs {
            pig.position.x.to_bits().hash(&mut hasher);
            pig.position.z.to_bits().hash(&mut hasher);
            pig.path.hash(&mut hasher);
            pig.idle_time.hash(&mut hasher);
            pig.damage.hash(&mut hasher);
            pig.panic.hash(&mut hasher);
        }
        for (tile, brains) in self.driver.stage().tiles() {
            tile.hash(&mut hasher);
            for entry in brains.iter() {
                entry.mob.hash(&mut hasher);
                if scope == HashScope::Everything {
                    entry.wake.hash(&mut hasher);
                }
                entry.brain.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    /// Sets the damage record of pig `index` as a hit in the last tick.
    pub fn hurt(&mut self, index: usize, cause: DamageCause) {
        let tick = self.driver.tick();
        if let Some(pig) = self.pigs.get_mut(index) {
            pig.damage = Some(DamageRecord { tick, cause });
            pig.idle_time = 0;
            self.facts.entry(pig.tile).or_default().push(Fact {
                mob: mob(index),
                kind: FactKind::Hurt,
            });
        }
    }

    /// The position of pig `index` and whether it walks.
    #[must_use]
    pub fn pig(&self, index: usize) -> Option<PenPig> {
        self.pigs.get(index).map(|pig| PenPig {
            position: pig.position,
            walk_speed: pig.walk.map(|walk| walk.per_tick),
            walk_target: pig.walk.map(|walk| walk.target),
            look: match pig.look {
                Look::Idle => PenLook::Idle,
                Look::At { .. } => PenLook::AtPlayer,
                Look::Around => PenLook::Around,
            },
            panic: pig.panic,
        })
    }

    /// Puts pig `index` at `position`.
    pub fn place_pig(&mut self, index: usize, position: DVec3) {
        if let Some(pig) = self.pigs.get_mut(index) {
            pig.position = position;
            pig.tile = TileKey::at(position);
            self.driver.relocate_all([(mob(index), pig.tile)]);
        }
    }

    /// Fills `pond` with water. A pig whose feet are in it is in water.
    pub fn flood(&mut self, pond: Pond) {
        self.ground.pond = Some(pond);
    }

    /// Sets whether pig `index` burns.
    pub fn set_on_fire(&mut self, index: usize, on_fire: bool) {
        if let Some(pig) = self.pigs.get_mut(index) {
            pig.on_fire = on_fire;
        }
    }

    /// Moves the players to `players`.
    pub fn set_players(&mut self, players: Vec<PlayerRow>) {
        self.players = players;
    }

    fn publish(&mut self) {
        let mut tiles: BTreeMap<TileKey, Vec<MobView>> = BTreeMap::new();
        for (index, pig) in self.pigs.iter().enumerate() {
            let in_water = self
                .ground
                .pond
                .is_some_and(|pond| pond.holds(BlockPos::from(pig.position)));
            tiles
                .entry(pig.tile)
                .or_default()
                .push(view(index, pig, in_water));
        }
        for (tile, views) in tiles {
            self.driver.publish_tile(tile, views);
        }
        for (tile, facts) in core::mem::take(&mut self.facts) {
            self.driver.publish_facts(tile, facts);
        }
        self.driver
            .publish_players(PlayerTable::new(self.players.iter().copied()));
    }

    fn apply(&mut self, decision: &Decision) {
        let Some(pig) = usize::try_from(decision.mob.0)
            .ok()
            .and_then(|index| self.pigs.get_mut(index))
        else {
            return;
        };
        let intents = decision.intents;
        match intents.path {
            Some(PathIntent::Walk {
                ticket,
                target,
                speed_modifier,
            }) => {
                let speed = f64::from(speed_modifier) * PIG_SPEED;
                pig.walk = Some(Walk {
                    target,
                    per_tick: speed * speed * GRASS_WALK,
                });
                pig.path = running(ticket);
            }
            Some(PathIntent::Clear { ticket }) => {
                pig.walk = None;
                pig.path = ended(ticket);
            }
            None => {}
        }
        match intents.look {
            Some(LookIntent::At { hold, .. }) => pig.look = Look::At { hold },
            Some(LookIntent::Around { .. }) => pig.look = Look::Around,
            Some(LookIntent::EndAround) if pig.look == Look::Around => pig.look = Look::Idle,
            Some(LookIntent::EndAround) | None => {}
        }
        if let Some(panic) = intents.panic {
            pig.panic = panic;
        }
    }

    fn hot_step(&mut self, tick: u64) {
        let seed = WorldSeed(self.config.seed);
        for (index, pig) in self.pigs.iter_mut().enumerate() {
            let mob = mob(index);
            let from = pig.tile;
            let mut facts: [Option<FactKind>; 3] = [None; 3];
            if let Some(walk) = pig.walk {
                let dx = walk.target.x - pig.position.x;
                let dz = walk.target.z - pig.position.z;
                let distance = libm::sqrt(dx * dx + dz * dz);
                if distance <= walk.per_tick {
                    pig.position = walk.target;
                    pig.walk = None;
                    pig.path.progress = Progress::Ended;
                    facts[0] = Some(FactKind::PathEnded);
                } else {
                    let share = walk.per_tick / distance;
                    pig.position.x += dx * share;
                    pig.position.z += dz * share;
                }
            }
            pig.look = match pig.look {
                Look::At { hold } if hold > 1 => Look::At { hold: hold - 1 },
                Look::At { .. } => Look::Idle,
                look => look,
            };
            let idle_time = match self.config.idle {
                IdleRule::AlwaysNear => 0,
                IdleRule::AlwaysFar => FAR_IDLE,
                IdleRule::FromPlayers => {
                    let near = self.players.iter().any(|player| {
                        player.is_qualified()
                            && player.position.distance_squared(pig.position)
                                < IDLE_RESET_RANGE * IDLE_RESET_RANGE
                    });
                    if near {
                        0
                    } else {
                        pig.idle_time.saturating_add(1)
                    }
                }
            };
            if idle_time == 0 && pig.idle_time >= STROLL_IDLE_LIMIT {
                facts[1] = Some(FactKind::IdleReset);
            }
            pig.idle_time = idle_time;
            if KeyedRandom::new(seed, &pig.key, tick, HURT).chance(self.config.hurt_chance) {
                pig.damage = Some(DamageRecord {
                    tick,
                    cause: DamageCause::Panic,
                });
                pig.idle_time = 0;
                facts[2] = Some(FactKind::Hurt);
            }
            pig.tile = TileKey::at(pig.position);
            if pig.tile != from {
                self.moves.push((mob, pig.tile));
            }
            for kind in facts.into_iter().flatten() {
                self.facts
                    .entry(pig.tile)
                    .or_default()
                    .push(Fact { mob, kind });
            }
        }
        self.driver.relocate_all(self.moves.drain(..));
    }
}

/// One pig of a pen, as a test reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenPig {
    pub position: DVec3,
    /// The blocks that the pig walks in each tick, while it walks.
    pub walk_speed: Option<f64>,
    /// The end of the walk, while the pig walks.
    pub walk_target: Option<DVec3>,
    pub look: PenLook,
    pub panic: Panic,
}

/// What a pig of a pen looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PenLook {
    Idle,
    AtPlayer,
    Around,
}

/// Which state a hash of a pen covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HashScope {
    /// The pigs, the brains and the wake tick of each brain.
    Everything,
    /// The pigs and the brains without their wake ticks, which differ between the wake modes.
    Decisions,
}

/// Runs the tile tasks one after the other.
pub fn run_tiles(work: &mut [TileWork<'_, PigBrain>], cx: &TickContext<'_, FlatGround>) {
    for task in work {
        task.run(cx);
    }
}

fn mob(index: usize) -> EntityId {
    EntityId(index as u64)
}

const fn running(ticket: Ticket) -> Applied {
    Applied {
        ticket,
        progress: Progress::Running,
    }
}

const fn ended(ticket: Ticket) -> Applied {
    Applied {
        ticket,
        progress: Progress::Ended,
    }
}

fn view(index: usize, pig: &HostPig, in_water: bool) -> MobView {
    let marks = [
        (pig.panic == Panic::Panicking, MobFlags::IN_PANIC),
        (in_water, MobFlags::IN_WATER),
        (pig.on_fire, MobFlags::ON_FIRE),
    ];
    let flags = marks.into_iter().filter(|(set, _)| *set).fold(
        MobFlags::ALIVE.union(MobFlags::ON_GROUND),
        |flags, (_, flag)| flags.union(flag),
    );
    MobView {
        mob: mob(index),
        key: pig.key,
        position: pig.position,
        eye_height: EntityTypeData::PIG.eye_height,
        flags,
        idle_time: pig.idle_time,
        damage: pig.damage,
        disabled: ControlFlags::NONE,
        path: pig.path,
    }
}

/// FNV-1a over the bytes that the state writes.
struct StateHasher(u64);

impl Default for StateHasher {
    fn default() -> Self {
        Self(0xCBF2_9CE4_8422_2325)
    }
}

impl Hasher for StateHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
}
