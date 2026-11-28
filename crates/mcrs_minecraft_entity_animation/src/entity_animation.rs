use glam::Vec3;
use mcrs_molang::{Expression, QuerySlot, QueryValues, ScalarVm, Subject, Value, Variables};

use crate::clip::{Clip, TimeUpdate};
use crate::entity_query::EntityQuery;
use crate::instance::{Instance, Phase, Step};
use crate::query_source::QuerySource;
use crate::rig::{AnimateTarget, Evaluation, Rig, Script};

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ScriptCache {
    pub inputs: Option<Vec<Option<u64>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntityAnimation {
    pub(crate) variables: Variables,
    pub(crate) queries: QueryValues,
    pub(crate) instances: Vec<Instance>,
    pub(crate) previous_clock: Option<f32>,
    pub(crate) clock: f32,
    pub(crate) delta_time: f32,
    pub(crate) initialized: bool,
    pub(crate) pre_animation: ScriptCache,
    pub(crate) scale_cache: ScriptCache,
    pub(crate) scale: Vec3,
}

impl EntityAnimation {
    #[must_use]
    pub fn new(rig: &Rig) -> Self {
        Self {
            variables: Variables::new(&rig.layout),
            queries: QueryValues::new(rig.queries.count),
            instances: vec![Instance::default(); rig.animate.len()],
            previous_clock: None,
            clock: 0.0,
            delta_time: 0.0,
            initialized: false,
            pre_animation: ScriptCache::default(),
            scale_cache: ScriptCache::default(),
            scale: Vec3::ONE,
        }
    }

    #[must_use]
    pub fn scale(&self) -> Vec3 {
        self.scale
    }

    #[must_use]
    pub fn variables(&self) -> &Variables {
        &self.variables
    }

    #[must_use]
    pub fn query_number(&self, slot: QuerySlot) -> Option<f32> {
        let raw = u32::try_from(self.queries.get(slot)?).ok()?;
        Some(f32::from_bits(raw))
    }

    #[must_use]
    pub fn instance(&self, entry: usize) -> Option<&Instance> {
        self.instances.get(entry)
    }

    pub fn set_weight(&mut self, entry: usize, weight: f32) {
        let clock = self.clock;
        let Some(instance) = self.instances.get_mut(entry) else {
            return;
        };
        if weight == 0.0 {
            *instance = Instance::default();
            return;
        }
        if matches!(instance.phase, Phase::Stopped | Phase::StartDelay) {
            instance.start(clock, 0.0);
        }
        instance.weight = weight;
    }

    pub fn set_time(&mut self, rig: &Rig, entry: usize, time: f32) {
        let clock = self.clock;
        let (Some(instance), Some(AnimateTarget::Clip(clip))) = (
            self.instances.get_mut(entry),
            rig.animate.get(entry).map(|entry| entry.target),
        ) else {
            return;
        };
        let Some(clip) = rig.clips.get(clip) else {
            return;
        };
        if instance.phase != Phase::Stopped {
            instance.phase = Phase::Playing;
            instance.set_time(clip, time, clock);
        }
    }

    pub fn update<S: QuerySource>(&mut self, rig: &Rig, vm: &mut ScalarVm, clock: f32, source: &S) {
        self.begin_frame(rig, clock, |query| source.query(query).map(Value::to_raw));
        self.run_scripts(rig, vm);
        self.update_play_list(rig, vm);
    }

    pub(crate) fn begin_frame(
        &mut self,
        rig: &Rig,
        clock: f32,
        mut source: impl FnMut(EntityQuery) -> Option<u64>,
    ) {
        self.delta_time = self
            .previous_clock
            .map_or(0.0, |previous| (clock - previous) / 20.0);
        self.previous_clock = Some(clock);
        self.clock = clock;
        for host in &rig.queries.host {
            self.set_query(host.slot, source(host.query));
        }
        self.set_number(rig.queries.delta_time, self.delta_time);
        self.set_number(rig.queries.anim_time, 0.0);
        self.set_number(rig.queries.key_frame_lerp_time, 0.0);
    }

    pub(crate) fn run_scripts(&mut self, rig: &Rig, vm: &mut ScalarVm) {
        if !self.initialized {
            self.initialized = true;
            if let Some(initialize) = &rig.initialize {
                self.number(vm, initialize, 0.0);
            }
        }
        if let Some(script) = &rig.pre_animation {
            let mut cache = core::mem::take(&mut self.pre_animation);
            if self.inputs_changed(script, &mut cache) {
                for expression in &script.expressions {
                    self.number(vm, expression, 0.0);
                }
            }
            self.pre_animation = cache;
        }
        let mut cache = core::mem::take(&mut self.scale_cache);
        if self.inputs_changed(&rig.scale, &mut cache) {
            let mut values = [1.0f32; 4];
            for (value, expression) in values.iter_mut().zip(&rig.scale.expressions) {
                *value = self.number(vm, expression, 0.0);
            }
            let [all, x, y, z] = values;
            self.scale = Vec3::new(all * x, all * y, all * z);
        }
        self.scale_cache = cache;
    }

    fn update_play_list(&mut self, rig: &Rig, vm: &mut ScalarVm) {
        for (index, entry) in rig.animate.iter().enumerate() {
            let weight = self.number(vm, &entry.weight, 0.0);
            self.update_instance(rig, vm, index, weight);
        }
    }

    pub(crate) fn update_instance(
        &mut self,
        rig: &Rig,
        vm: &mut ScalarVm,
        index: usize,
        weight: f32,
    ) {
        let (Some(mut instance), Some(AnimateTarget::Clip(clip))) = (
            self.instances.get(index).copied(),
            rig.animate.get(index).map(|entry| entry.target),
        ) else {
            return;
        };
        let Some(clip) = rig.clips.get(clip) else {
            return;
        };
        if weight == 0.0 || !weight.is_finite() {
            instance = Instance::default();
        } else {
            let slots = &rig.queries;
            let started = instance.phase == Phase::Stopped;
            if started {
                self.set_number(slots.anim_time, 0.0);
                self.set_number(slots.delta_time, 0.0);
                let delay = self.number(vm, &clip.start_delay, 0.0);
                instance.start(self.clock, delay);
            }
            let step = Step {
                delta_time: if started { 0.0 } else { self.delta_time },
                clock: self.clock,
            };
            let in_loop_delay = instance.phase == Phase::LoopDelay;
            if instance.run_delay(step) {
                if in_loop_delay {
                    instance.first_update = false;
                } else {
                    self.advance_time(vm, rig, clip, &mut instance, step);
                }
            }
            self.set_number(slots.delta_time, self.delta_time);
            self.set_number(slots.anim_time, instance.time);
            instance.weight = weight * self.number(vm, &clip.blend_weight, 0.0);
            self.set_number(slots.anim_time, 0.0);
        }
        if let Some(held) = self.instances.get_mut(index) {
            *held = instance;
        }
    }

    fn advance_time(
        &mut self,
        vm: &mut ScalarVm,
        rig: &Rig,
        clip: &Clip,
        instance: &mut Instance,
        step: Step,
    ) {
        let slots = &rig.queries;
        let time = match &clip.time_update {
            TimeUpdate::Clock => instance.clock_time(step),
            TimeUpdate::Expression(expression) => {
                self.set_number(slots.anim_time, instance.time);
                self.set_number(slots.delta_time, instance.delta_time(step));
                self.number(vm, expression, 0.0)
            }
        };
        if instance.set_time(clip, time, self.clock) {
            self.set_number(slots.anim_time, instance.time);
            let delay = self.number(vm, &clip.loop_delay, 0.0);
            instance.begin_loop_delay(delay);
        }
    }

    fn inputs_changed(&self, script: &Script, cache: &mut ScriptCache) -> bool {
        let Evaluation::OnChange { queries, variables } = &script.evaluation else {
            return true;
        };
        let inputs = queries
            .iter()
            .map(|slot| self.queries.get(*slot))
            .chain(variables.iter().map(|slot| self.variables.get(*slot)));
        if let Some(held) = &cache.inputs
            && held.iter().copied().eq(inputs.clone())
        {
            return false;
        }
        cache.inputs = Some(inputs.collect());
        true
    }

    pub(crate) fn set_query(&mut self, slot: QuerySlot, raw: Option<u64>) {
        self.queries.set(slot, raw);
    }

    pub(crate) fn set_number(&mut self, slot: Option<QuerySlot>, value: f32) {
        if let Some(slot) = slot {
            self.set_query(slot, Some(u64::from(value.to_bits())));
        }
    }

    pub(crate) fn number(&mut self, vm: &mut ScalarVm, expression: &Expression, this: f32) -> f32 {
        match expression {
            Expression::Constant(value) => value.as_number(),
            Expression::Program(program) => vm
                .run(
                    program,
                    &mut Subject {
                        variables: &mut self.variables,
                        queries: &self.queries,
                        this,
                    },
                )
                .as_number(),
        }
    }
}
