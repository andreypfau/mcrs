use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::{Or, With};
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use glam::DVec3;
use mcrs_minecraft_damage::DamageTypeRules;
use mcrs_minecraft_damage::Defence;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_damage::HitRecord;
use mcrs_minecraft_damage::HurtRules;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine_entity_motion::GridSlot;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion::StepOutcome;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::WriterKey;
use mcrs_minecraft_physics::GridRow;

use crate::DamageRequest;
use crate::Damaged;
use crate::Died;
use crate::PendingHits;
use crate::vitals_work::VitalsWork;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct HitOrder {
    target: EntityUuid,
    writer: WriterKey,
    tick: u64,
    sequence: u32,
    index: usize,
}

type Targets<'w, 's> = Query<
    'w,
    's,
    (
        Place<'static>,
        &'static mut Vitals,
        &'static mut HitRecord,
        Option<&'static Defence>,
        Option<&'static GridSlot>,
        Option<&'static StepOutcome>,
    ),
    Or<(With<MotionState>, (With<EntityUuid>, With<Transform>))>,
>;

type Place<'a> = (
    Option<&'a MotionState>,
    Option<&'a EntityUuid>,
    Option<&'a Transform>,
);

fn key((motion, uuid, _): Place) -> Option<EntityUuid> {
    motion.map(|motion| motion.0.key).or(uuid.copied())
}

fn position((motion, _, transform): Place) -> Option<DVec3> {
    motion
        .map(|motion| motion.0.body.position)
        .or(transform.map(|transform| transform.translation))
}

#[derive(Default)]
pub(crate) struct ApplyScratch {
    order: Vec<HitOrder>,
    dead: Vec<GridRow>,
}

pub(crate) struct Reports<'a, 'd, 'e> {
    damaged: MessageWriter<'d, Damaged>,
    died: MessageWriter<'e, Died>,
    work: &'a mut VitalsWork,
}

#[expect(
    clippy::too_many_arguments,
    reason = "a system takes each resource that it reads"
)]
pub(crate) fn apply_hits(
    mut pending: ResMut<PendingHits>,
    types: Res<DamageTypeRules>,
    clock: Option<Res<BirthClock>>,
    mut targets: Targets,
    damaged: MessageWriter<Damaged>,
    died: MessageWriter<Died>,
    mut work: ResMut<VitalsWork>,
    mut scratch: Local<ApplyScratch>,
) {
    let tick = clock.map(|clock| clock.0);
    let ApplyScratch { order, dead } = &mut *scratch;
    order.clear();
    dead.clear();
    for (index, request) in pending.requests.iter().enumerate() {
        let Some(target) = targets
            .get(request.target)
            .ok()
            .and_then(|(place, ..)| key(place))
        else {
            continue;
        };
        order.push(HitOrder {
            target,
            writer: request.writer,
            tick: request.tick,
            sequence: request.sequence,
            index,
        });
    }
    order.sort_unstable();
    let mut reports = Reports {
        damaged,
        died,
        work: &mut work,
    };
    for group in order.chunk_by(|left, right| left.target == right.target) {
        apply_group(
            group,
            &pending.requests,
            &types,
            tick,
            &mut targets,
            dead,
            &mut reports,
        );
    }
    let work = &mut reports.work.0;
    work.sort_unstable();
    work.dedup();
    pending.requests.clear();
}

fn apply_group(
    group: &[HitOrder],
    requests: &[DamageRequest],
    types: &DamageTypeRules,
    tick: Option<u64>,
    targets: &mut Targets,
    dead: &mut Vec<GridRow>,
    reports: &mut Reports<'_, '_, '_>,
) {
    let crowd_row = group
        .iter()
        .filter_map(|order| requests.get(order.index))
        .find_map(|request| request.crowd.map(|crowd| crowd.own));
    let mut counted = false;
    for order in group {
        let Some(request) = requests.get(order.index) else {
            continue;
        };
        if request.crowd.is_some_and(|crowd| !crowd.overcrowded(dead)) {
            continue;
        }
        let Ok((place, mut vitals, mut record, defence, slot, outcome)) =
            targets.get_mut(request.target)
        else {
            continue;
        };
        let Some(position) = position(place) else {
            continue;
        };
        let mut next_vitals = *vitals;
        let mut next_record = *record;
        let report = mcrs_minecraft_damage::hurt(
            &mut next_vitals,
            &mut next_record,
            &defence.copied().unwrap_or_default(),
            types,
            &HurtRules::default(),
            request.hit,
            request.tick,
        );
        vitals.set_if_neq(next_vitals);
        record.set_if_neq(next_record);
        counted |= next_vitals.is_counting();
        if report.outcome == HitOutcome::Rejected {
            continue;
        }
        let source = request.hit.source;
        reports.damaged.write(Damaged {
            target: request.target,
            source,
            report,
        });
        if report.died {
            reports.died.write(Died {
                target: request.target,
                position,
                source,
            });
            if let Some(row) = crowd_row.or_else(|| counted_row(slot, outcome, tick)) {
                dead.push(row);
            }
        }
    }
    if counted && let Some(first) = group.first().and_then(|order| requests.get(order.index)) {
        reports.work.0.push(first.target);
    }
}

fn counted_row(
    slot: Option<&GridSlot>,
    outcome: Option<&StepOutcome>,
    tick: Option<u64>,
) -> Option<GridRow> {
    let row = slot?.row()?;
    Some(match outcome {
        Some(outcome) if tick.is_some_and(|tick| outcome.is_of(tick)) => GridRow {
            position: outcome.start,
            ..row
        },
        _ => row,
    })
}
