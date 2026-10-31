use crate::ticks::{A, B, BUDGET, POLICIES, Policy, rs, saved};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_simulation::{
    Collected, GameTick, Queued, SavedTick, ScheduledTick, SubTick, TickKey, TickPriority,
    TickQueue,
};
use mcrs_voxel::{BlockPos, SectionPos};
use std::collections::HashMap;

type Key3 = (i32, i32, i32);
type Pair = (BlockPos, u16);

#[derive(Clone, Copy, Debug)]
struct Pending {
    pos: BlockPos,
    kind: u16,
    key: TickKey,
}

#[derive(Default)]
struct Model {
    active: Vec<Pending>,
    held: HashMap<Key3, Vec<SavedTick<u16>>>,
    pairs: HashMap<Pair, u32>,
}

fn section_key(pos: BlockPos) -> Key3 {
    let section = SectionPos::from(pos);
    (section.x, section.y, section.z)
}

fn column_of(pos: BlockPos) -> (i32, i32) {
    let section = SectionPos::from(pos);
    (section.x, section.z)
}

impl Model {
    fn add_pair(&mut self, pair: Pair) {
        *self.pairs.entry(pair).or_default() += 1;
    }

    fn knows(&self, pos: BlockPos, kind: u16, sequence: SubTick) -> bool {
        let same = |p: BlockPos, k: u16, s: SubTick| p == pos && k == kind && s == sequence;
        self.active
            .iter()
            .any(|tick| same(tick.pos, tick.kind, tick.key.sequence))
            || self
                .held
                .values()
                .flatten()
                .any(|tick| same(tick.pos, tick.kind, tick.sequence))
    }

    fn collected(&mut self, ran: &Collected<u16>, context: &str) -> TickKey {
        let matches: Vec<usize> = (0..self.active.len())
            .filter(|&i| {
                let tick = self.active[i];
                tick.pos == ran.pos && tick.kind == ran.kind && tick.key.sequence == ran.sequence
            })
            .collect();
        assert_eq!(matches.len(), 1, "{context}: {ran:?}");
        let tick = self.active.swap_remove(matches[0]);
        let pair = (tick.pos, tick.kind);
        let count = self.pairs.get_mut(&pair).expect("a pending pair");
        *count -= 1;
        if *count == 0 {
            self.pairs.remove(&pair);
        }
        tick.key
    }

    fn len(&self) -> usize {
        self.active.len() + self.held.values().map(Vec::len).sum::<usize>()
    }
}

struct Generator {
    random: LegacyRandom,
    recent: Vec<BlockPos>,
}

impl Generator {
    fn below(&mut self, bound: i32) -> i32 {
        self.random.next_i32_bound(bound)
    }

    fn kind(&mut self) -> u16 {
        if self.below(4) == 0 { B } else { A }
    }

    fn fresh(&mut self) -> BlockPos {
        let shift = |generator: &mut Self, span: i32| {
            if generator.below(4) == 0 { -span } else { 0 }
        };
        let x = self.below(48) + shift(self, 48);
        let z = self.below(32) + shift(self, 32);
        let pos = BlockPos::new(x, self.below(48) - 16, z);
        if self.recent.len() == 16 {
            self.recent.remove(0);
        }
        self.recent.push(pos);
        pos
    }

    fn pos(&mut self) -> BlockPos {
        if !self.recent.is_empty() && self.below(3) == 0 {
            let at = self.below(self.recent.len() as i32) as usize;
            return self.recent[at];
        }
        self.fresh()
    }
}

struct Run {
    generator: Generator,
    queue: TickQueue<u16>,
    spare: TickQueue<u16>,
    model: Model,
    activations: Vec<(i64, Key3)>,
    counter: u32,
}

impl Run {
    fn schedule(
        &mut self,
        now: i64,
        pos: BlockPos,
        kind: u16,
        due: i64,
        priority: i32,
        context: &str,
    ) {
        let key = TickKey {
            due: GameTick(due),
            priority: TickPriority::from_saved(priority),
            sequence: SubTick::runtime(GameTick(now), self.counter),
        };
        self.counter += 1;
        let expected = if self.model.pairs.contains_key(&(pos, kind)) {
            Queued::Duplicate
        } else {
            Queued::Accepted
        };
        let queued = self.queue.schedule(ScheduledTick { pos, kind, key });
        assert_eq!(queued, expected, "{context}: {pos}");
        if queued == Queued::Accepted {
            self.model.active.push(Pending { pos, kind, key });
            self.model.add_pair((pos, kind));
        }
    }

    fn schedule_batch(&mut self, now: i64, context: &str) {
        for _ in 0..self.generator.below(10) {
            let pos = self.generator.pos();
            let kind = self.generator.kind();
            let due = now + i64::from(self.generator.below(8)) - 2;
            let priority = self.generator.below(7) - 3;
            self.schedule(now, pos, kind, due, priority, context);
        }
    }

    fn churn_one_head(&mut self, now: i64, context: &str) {
        let origin = self.generator.fresh();
        let corner = |axis: i32| axis - axis.rem_euclid(16);
        for i in 0..80 {
            let pos = BlockPos::new(
                corner(origin.x) + i % 16,
                corner(origin.y) + i / 16,
                corner(origin.z),
            );
            self.schedule(now, pos, A, now + 100 - i64::from(i), 0, context);
        }
    }

    fn hold_batch(&mut self, now: i64) {
        let count = self.generator.below(4) as u32 + 1;
        let mut pos = self.generator.pos();
        let kind = self.generator.kind();
        for k in 0..count {
            let sequence = rs(k, count);
            if self.generator.below(3) != 0 {
                pos = self.generator.pos();
            }
            while self.model.knows(pos, kind, sequence) {
                pos = self.generator.fresh();
            }
            let tick = SavedTick {
                priority: TickPriority::from_saved(self.generator.below(3) - 1),
                ..saved(pos, kind, self.generator.below(7) - 3, sequence)
            };
            self.queue.hold_saved(tick);
            self.model
                .held
                .entry(section_key(pos))
                .or_default()
                .push(tick);
            self.model.add_pair((pos, kind));
            let at = now + 1 + i64::from(self.generator.below(4));
            self.activations.push((at, section_key(pos)));
        }
    }

    fn activate(&mut self, now: i64, key: Key3) {
        let (x, y, z) = key;
        self.queue.activate(SectionPos::new(x, y, z), GameTick(now));
        for tick in self.model.held.remove(&key).unwrap_or_default() {
            let key = TickKey {
                due: GameTick(now + i64::from(tick.delay)),
                priority: tick.priority,
                sequence: tick.sequence,
            };
            self.model.active.push(Pending {
                pos: tick.pos,
                kind: tick.kind,
                key,
            });
        }
    }

    fn move_a_section_away_and_back(&mut self) {
        let mut sections: Vec<_> = self.queue.sections().map(|s| (s.x, s.y, s.z)).collect();
        sections.sort();
        if sections.is_empty() {
            return;
        }
        let (x, y, z) = sections[self.generator.below(sections.len() as i32) as usize];
        let section = SectionPos::new(x, y, z);
        let ticks = self.queue.take_section(section).expect("a listed section");
        self.spare
            .put_section(ticks)
            .expect("the spare queue is empty");
        let ticks = self.spare.take_section(section).expect("just put");
        self.queue.put_section(ticks).expect("just taken");
    }

    fn collect_and_check(&mut self, policy: Policy, now: i64, context: &str) {
        let budget = self.generator.below(12) as u32 + 1;
        let excluded = self.generator.below(5);
        let eligible = |s: SectionPos| (s.x ^ s.y ^ s.z).rem_euclid(4) != excluded;
        let mut unbudgeted = self.queue.clone();
        let ran = policy.collect_within(&mut self.queue, now, budget, eligible);
        let all = policy.collect_within(&mut unbudgeted, now, BUDGET, eligible);
        assert_eq!(ran[..], all[..ran.len()], "{context}");
        assert!(ran.len() <= budget as usize, "{context}");

        let due_before: Vec<Pending> = self
            .model
            .active
            .iter()
            .filter(|tick| tick.key.due.0 <= now && eligible(SectionPos::from(tick.pos)))
            .copied()
            .collect();
        let keys: Vec<TickKey> = ran
            .iter()
            .map(|tick| self.model.collected(tick, context))
            .collect();
        for tick in &ran {
            assert!(eligible(SectionPos::from(tick.pos)), "{context}");
        }
        check_orders(policy, &ran, &keys, context);
        if matches!(policy, Policy::Vanilla) {
            check_vanilla_rivals(&ran, &keys, due_before, context);
        }
        if ran.len() < budget as usize {
            let left_due = self
                .model
                .active
                .iter()
                .any(|tick| tick.key.due.0 <= now && eligible(SectionPos::from(tick.pos)));
            assert!(!left_due, "{context}: a due tick was left behind");
        }
        assert_eq!(self.queue.len(), self.model.len(), "{context}");
        let next_due = self.model.active.iter().map(|tick| tick.key.due).min();
        assert_eq!(self.queue.next_due(), next_due, "{context}");
    }
}

fn check_orders(policy: Policy, ran: &[Collected<u16>], keys: &[TickKey], context: &str) {
    let mut groups: HashMap<Key3, Vec<TickKey>> = HashMap::new();
    for (tick, key) in ran.iter().zip(keys) {
        let (x, y, z) = section_key(tick.pos);
        groups.entry((x, y, z)).or_default().push(*key);
        if matches!(policy, Policy::Vanilla) {
            groups.entry((x, i32::MIN, z)).or_default().push(*key);
        }
    }
    for group in groups.values() {
        assert!(group.is_sorted(), "{context}: {group:?}");
    }
    if matches!(policy, Policy::Semantic) {
        assert!(keys.is_sorted(), "{context}");
    }
}

fn check_vanilla_rivals(
    ran: &[Collected<u16>],
    keys: &[TickKey],
    mut remaining: Vec<Pending>,
    context: &str,
) {
    for (tick, key) in ran.iter().zip(keys) {
        remaining.retain(|pending| {
            !(pending.pos == tick.pos
                && pending.kind == tick.kind
                && pending.key.sequence == tick.sequence)
        });
        let mut heads: HashMap<(i32, i32), TickKey> = HashMap::new();
        for pending in &remaining {
            let head = heads.entry(column_of(pending.pos)).or_insert(pending.key);
            *head = (*head).min(pending.key);
        }
        let own = column_of(tick.pos);
        for (column, head) in heads {
            if column != own {
                assert!(
                    (key.priority, key.sequence) <= (head.priority, head.sequence),
                    "{context}: {tick:?} ran ahead of the head of column {column:?}"
                );
            }
        }
    }
}

fn random_collections_keep_the_observable_rules(seed: u64, phases: i64) {
    for policy in POLICIES {
        let mut run = Run {
            generator: Generator {
                random: LegacyRandom::new(seed),
                recent: Vec::new(),
            },
            queue: TickQueue::new(),
            spare: TickQueue::new(),
            model: Model::default(),
            activations: Vec::new(),
            counter: 0,
        };
        for now in 0..phases {
            let context = format!("{policy:?} seed {seed} at {now}");
            run.schedule_batch(now, &context);
            if run.generator.below(8) == 0 {
                run.hold_batch(now);
            }
            let due: Vec<_> = run
                .activations
                .iter()
                .filter(|(at, _)| *at == now)
                .map(|(_, key)| *key)
                .collect();
            for key in due {
                run.activate(now, key);
            }
            if run.generator.below(10) == 0 {
                run.move_a_section_away_and_back();
            }
            if run.generator.below(40) == 0 {
                run.churn_one_head(now, &context);
            }
            run.collect_and_check(policy, now, &context);
        }
        let held: Vec<_> = run.model.held.keys().copied().collect();
        for key in held {
            run.activate(phases, key);
        }
        let rest = policy.collect(&mut run.queue, phases + 256);
        assert_eq!(rest.len(), run.model.len(), "{policy:?} seed {seed}");
        assert!(run.queue.is_empty());
    }
}

#[test]
fn random_collections_keep_the_observable_rules_for_a_few_seeds() {
    for seed in 0..4 {
        random_collections_keep_the_observable_rules(seed, 100);
    }
}

mod exhaustive {
    #[test]
    fn random_collections_keep_the_observable_rules_over_many_seeds() {
        for seed in 0..200 {
            super::random_collections_keep_the_observable_rules(seed, 200);
        }
    }
}
