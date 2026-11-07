use glam::Vec3;
use mcrs_molang::{
    BatchOutput, BatchQueries, BatchSubjects, BatchVariables, BatchVm, Expression, Program,
    QuerySlot, ScalarVm, Value,
};

use crate::channel::Channel;
use crate::entity_animation::EntityAnimation;
use crate::pose::{ChannelKind, Pose, PoseBone};
use crate::pose_binding::PoseBinding;
use crate::query_source::QuerySource;
use crate::rig::{AnimateTarget, Rig};

/// The frames of up to `N` entities of one rig at a time, with each Molang program of a weight
/// or of a channel run once for all lanes by the batch VM. The results are those of
/// [`EntityAnimation::update`] and [`EntityAnimation::pose`].
#[derive(Debug)]
pub struct PoseBatch<const N: usize> {
    batch: BatchVm<N>,
    scalar: ScalarVm,
    queries: BatchQueries<N>,
    /// Stays empty: a rig with variables runs each lane on the scalar VM.
    variables: BatchVariables<N>,
    this: [f32; N],
    values: [f32; N],
    times: [Option<f32>; N],
}

/// The entities of one batch run and their inputs, lane by lane.
#[derive(Debug)]
pub struct BatchLanes<'a, S> {
    pub entities: &'a mut [EntityAnimation],
    /// The entity clock of each entity, in ticks.
    pub clocks: &'a [f32],
    pub sources: &'a [S],
    pub poses: &'a mut [Pose],
}

impl<const N: usize> PoseBatch<N> {
    #[must_use]
    pub fn new(rig: &Rig) -> Self {
        Self {
            batch: BatchVm::new(),
            scalar: ScalarVm::new(),
            queries: BatchQueries::new(rig.queries.count),
            variables: BatchVariables::new(&rig.layout),
            this: [0.0; N],
            values: [0.0; N],
            times: [None; N],
        }
    }

    /// Updates and poses each entity of `lanes`. The four slices have one element for each
    /// entity; the shortest one sets the number of entities.
    pub fn run<S: QuerySource>(
        &mut self,
        rig: &Rig,
        binding: &PoseBinding,
        lanes: BatchLanes<'_, S>,
    ) {
        let BatchLanes {
            entities,
            clocks,
            sources,
            poses,
        } = lanes;
        let blocks = entities
            .chunks_mut(N)
            .zip(clocks.chunks(N))
            .zip(sources.chunks(N))
            .zip(poses.chunks_mut(N));
        for (((entities, clocks), sources), poses) in blocks {
            self.block(rig, binding, entities, clocks, sources, poses);
        }
    }

    fn block<S: QuerySource>(
        &mut self,
        rig: &Rig,
        binding: &PoseBinding,
        entities: &mut [EntityAnimation],
        clocks: &[f32],
        sources: &[S],
        poses: &mut [Pose],
    ) {
        let lanes = entities
            .len()
            .min(clocks.len())
            .min(sources.len())
            .min(poses.len());
        if rig.layout.number_slots() + rig.layout.bits_slots() > 0 {
            let inputs = entities.iter_mut().zip(clocks).zip(sources).zip(poses);
            for (((entity, clock), source), pose) in inputs {
                entity.update(rig, &mut self.scalar, *clock, source);
                entity.pose(rig, &mut self.scalar, binding, pose);
            }
            return;
        }
        for ((entity, clock), source) in entities.iter_mut().zip(clocks).zip(sources) {
            entity.begin_frame(rig, *clock, |query| source.query(query).map(Value::to_raw));
            entity.run_scripts(rig, &mut self.scalar);
        }
        self.load(entities);
        for (index, entry) in rig.animate.iter().enumerate() {
            self.evaluate(&entry.weight, lanes);
            for (lane, entity) in entities.iter_mut().enumerate() {
                let weight = self.values.get(lane).copied().unwrap_or(0.0);
                entity.update_instance(rig, &mut self.scalar, index, weight);
            }
        }
        for pose in poses.iter_mut() {
            pose.bones.clear();
            pose.bones.resize(rig.bones.len(), PoseBone::REST);
        }
        self.load(entities);
        for (index, entry) in rig.animate.iter().enumerate() {
            let AnimateTarget::Clip(clip) = entry.target;
            let Some(clip) = rig.clips.get(clip) else {
                continue;
            };
            for (lane, time) in self.times.iter_mut().enumerate() {
                *time = entities
                    .get(lane)
                    .and_then(|entity| entity.instances.get(index))
                    .filter(|instance| instance.weight != 0.0)
                    .and_then(|instance| instance.sample_time(clip));
                let anim_time = entities
                    .get(lane)
                    .and_then(|entity| entity.instances.get(index))
                    .map_or(0.0, |instance| instance.time);
                set(&mut self.queries, lane, rig.queries.anim_time, anim_time);
            }
            if self.times.iter().take(lanes).all(Option::is_none) {
                continue;
            }
            for clip_bone in &clip.bones {
                let rest = binding.rest_rotation(clip_bone.bone);
                for (pose, time) in poses.iter_mut().zip(self.times).take(lanes) {
                    if let (Some(_), Some(bone)) = (time, pose.bones.get_mut(clip_bone.bone)) {
                        clip_bone.begin(clip, bone);
                    }
                }
                for (kind, channel) in clip_bone.channels() {
                    let Some(channel) = channel else {
                        continue;
                    };
                    let shared = shared_value(channel)
                        .filter(|_| self.times.iter().take(lanes).all(Option::is_some));
                    let target = ChannelTarget {
                        kind,
                        rest,
                        bone: clip_bone.bone,
                        entry: index,
                    };
                    match shared {
                        Some(value) => self.shared_channel(rig, value, target, entities, poses),
                        None => self.lane_channel(rig, channel, target, entities, poses),
                    }
                }
            }
        }
        for entity in entities.iter_mut() {
            entity.set_number(rig.queries.anim_time, 0.0);
            entity.set_number(rig.queries.key_frame_lerp_time, 0.0);
        }
    }

    /// A channel whose value does not depend on the time: each axis runs once for all lanes.
    fn shared_channel(
        &mut self,
        rig: &Rig,
        value: &[Expression; 3],
        target: ChannelTarget,
        entities: &[EntityAnimation],
        poses: &mut [Pose],
    ) {
        let lanes = entities.len().min(poses.len());
        let mut samples = [[0.0f32; 3]; N];
        for lane in 0..lanes {
            set(
                &mut self.queries,
                lane,
                rig.queries.key_frame_lerp_time,
                0.0,
            );
        }
        for (axis, expression) in value.iter().enumerate() {
            for (this, pose) in self.this.iter_mut().zip(poses.iter()) {
                *this = pose
                    .bones
                    .get(target.bone)
                    .map_or(0.0, |bone| target.kind.current(bone, target.rest, axis));
            }
            self.evaluate(expression, lanes);
            for (sample, value) in samples.iter_mut().zip(&self.values) {
                if let Some(slot) = sample.get_mut(axis) {
                    *slot = *value;
                }
            }
        }
        for ((entity, pose), sample) in entities.iter().zip(poses.iter_mut()).zip(samples) {
            let weight = entity
                .instances
                .get(target.entry)
                .map_or(0.0, |instance| instance.weight);
            if let Some(bone) = pose.bones.get_mut(target.bone) {
                target.kind.apply(bone, sample, weight);
            }
        }
    }

    /// A channel whose keyframes depend on the time of each lane: each lane samples on its own.
    fn lane_channel(
        &mut self,
        rig: &Rig,
        channel: &Channel,
        target: ChannelTarget,
        entities: &mut [EntityAnimation],
        poses: &mut [Pose],
    ) {
        let ChannelTarget {
            kind,
            rest,
            bone,
            entry,
        } = target;
        for ((entity, pose), time) in entities.iter_mut().zip(poses.iter_mut()).zip(self.times) {
            let (Some(time), Some(instance)) = (time, entity.instances.get(entry).copied()) else {
                continue;
            };
            let (Some(segment), Some(bone)) = (channel.segment(time), pose.bones.get_mut(bone))
            else {
                continue;
            };
            entity.set_number(rig.queries.anim_time, instance.time);
            entity.set_number(rig.queries.key_frame_lerp_time, segment.alpha());
            let before = *bone;
            let scalar = &mut self.scalar;
            let sample = channel.sample(segment, |expression, axis| {
                entity.number(scalar, expression, kind.current(&before, rest, axis))
            });
            kind.apply(bone, sample, instance.weight);
        }
    }

    /// Runs an expression for the first `lanes` lanes into `values`.
    fn evaluate(&mut self, expression: &Expression, lanes: usize) {
        match expression {
            Expression::Constant(value) => self.values = [value.as_number(); N],
            Expression::Program(program) => self.run_program(program, lanes),
        }
    }

    fn run_program(&mut self, program: &Program, lanes: usize) {
        let mut subjects = BatchSubjects {
            variables: &mut self.variables,
            queries: &self.queries,
            this: &self.this,
            lanes,
        };
        if let BatchOutput::Numbers(numbers) = self.batch.run(program, &mut subjects) {
            self.values = *numbers;
        } else {
            self.values = [0.0; N];
        }
    }

    /// Copies the query values of each entity into the lanes.
    fn load(&mut self, entities: &[EntityAnimation]) {
        for (lane, entity) in entities.iter().enumerate() {
            for (index, raw) in entity.raw_queries.iter().enumerate() {
                let Ok(slot) = u16::try_from(index) else {
                    break;
                };
                self.queries.set(lane, QuerySlot(slot), *raw);
            }
        }
    }
}

/// Where a channel of a clip goes.
#[derive(Clone, Copy, Debug)]
struct ChannelTarget {
    kind: ChannelKind,
    /// The rest rotation of the bone in file space, for `this`.
    rest: Vec3,
    bone: usize,
    /// The element of the `animate` list that plays the clip.
    entry: usize,
}

fn set<const N: usize>(
    queries: &mut BatchQueries<N>,
    lane: usize,
    slot: Option<QuerySlot>,
    value: f32,
) {
    if let Some(slot) = slot {
        queries.set_number(lane, slot, value);
    }
}

/// The value of a channel with one keyframe whose two sides are the same expression.
fn shared_value(channel: &Channel) -> Option<&[Expression; 3]> {
    let [key] = channel.keyframes.as_slice() else {
        return None;
    };
    (key.pre == key.post).then_some(&key.post)
}
