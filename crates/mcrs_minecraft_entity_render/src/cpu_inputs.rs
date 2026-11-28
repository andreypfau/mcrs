use mcrs_minecraft_entity_animation::{AnimateTarget, EntityAnimation, Rig};

use crate::pose_program::InputLayout;
use crate::records::{CLIP_PLAYING, ClipSlot, InputRecord};

#[must_use]
pub fn input_record(
    layout: &InputLayout,
    rig: &Rig,
    animation: &EntityAnimation,
    clock: f32,
) -> InputRecord {
    let mut record = InputRecord::default();
    for (index, (entry, inputs)) in rig.animate.iter().zip(&layout.entries).enumerate() {
        let clip = match entry.target {
            AnimateTarget::Clip(clip) => rig.clips.get(clip),
            _ => None,
        };
        let (Some(clip), Some(instance)) = (clip, animation.instance(index)) else {
            continue;
        };
        let plays = instance.sample_time(clip).is_some();
        let weight = if plays { instance.weight } else { 0.0 };
        if let Some(value) = inputs.weight.and_then(|at| record.values.get_mut(at)) {
            *value = if weight.is_finite() { weight } else { 0.0 };
        }
        if let Some(value) = inputs.time.and_then(|at| record.values.get_mut(at)) {
            *value = instance.time;
        }
        if let Some(slot) = inputs.clip.and_then(|at| record.clips.get_mut(at)) {
            *slot = ClipSlot {
                clip: index as u16,
                flags: if plays { CLIP_PLAYING } else { 0 },
                start: if plays {
                    clock - instance.time * 20.0
                } else {
                    0.0
                },
            };
        }
    }
    for input in &layout.queries {
        if let Some(value) = record.values.get_mut(input.value) {
            *value = animation
                .query_number(input.slot)
                .filter(|number| number.is_finite())
                .unwrap_or(0.0);
        }
    }
    record
}
