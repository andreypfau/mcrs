use mcrs_minecraft_damage::ActiveEffect;
use mcrs_minecraft_engine::player_state::EffectData;
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;

use crate::components::PlayerEffectsHidden;

fn active_from_data(effect: MobEffect, data: &EffectData) -> ActiveEffect {
    ActiveEffect {
        effect,
        amplifier: data.amplifier,
        duration: duration_from_file(data.duration),
        ambient: data.ambient,
        show_particles: data.show_particles,
        show_icon: data.show_icon.unwrap_or(data.show_particles),
    }
}

fn duration_from_file(duration: i32) -> u32 {
    if duration < 0 {
        u32::MAX
    } else {
        u32::try_from(duration).unwrap_or(u32::MAX)
    }
}

fn duration_to_file(duration: u32) -> i32 {
    if duration == u32::MAX {
        -1
    } else {
        i32::try_from(duration).unwrap_or(i32::MAX)
    }
}

fn data_from_active(active: &ActiveEffect, hidden: Option<EffectData>) -> EffectData {
    EffectData {
        amplifier: active.amplifier,
        duration: duration_to_file(active.duration),
        ambient: active.ambient,
        show_particles: active.show_particles,
        show_icon: Some(active.show_icon),
        hidden_effect: hidden.map(Box::new),
    }
}

fn is_shorter(shorter: u32, longer: u32) -> bool {
    shorter != u32::MAX && (longer == u32::MAX || shorter < longer)
}

pub(crate) fn merge_hidden(
    previous: Option<ActiveEffect>,
    added: ActiveEffect,
    hidden: &mut PlayerEffectsHidden,
) {
    let Some(previous) = previous else {
        return;
    };
    if added.amplifier > previous.amplifier {
        if is_shorter(added.duration, previous.duration) {
            let nested = hidden.0.remove(&previous.effect);
            hidden
                .0
                .insert(previous.effect, data_from_active(&previous, nested));
        } else {
            hidden.0.remove(&previous.effect);
        }
    } else if is_shorter(previous.duration, added.duration) && previous.amplifier != added.amplifier
    {
        if let Some(existing) = hidden.0.get_mut(&previous.effect) {
            merge_into_data(existing, &added);
        } else {
            hidden
                .0
                .insert(previous.effect, data_from_active(&added, None));
        }
    }
}

fn merge_into_data(existing: &mut EffectData, added: &ActiveEffect) {
    let existing_active = active_from_data(added.effect, existing);
    if added.amplifier > existing_active.amplifier {
        let previous_hidden = existing.hidden_effect.take().map(|nested| *nested);
        let nested = is_shorter(added.duration, existing_active.duration)
            .then(|| data_from_active(&existing_active, previous_hidden));
        *existing = data_from_active(added, nested);
    } else if is_shorter(existing_active.duration, added.duration)
        && existing_active.amplifier != added.amplifier
    {
        if let Some(nested) = &mut existing.hidden_effect {
            merge_into_data(nested, added);
        } else {
            existing.hidden_effect = Some(Box::new(data_from_active(added, None)));
        }
    }
}

pub(crate) fn tick_hidden(hidden: &mut PlayerEffectsHidden) {
    for data in hidden.0.values_mut() {
        tick_effect_data(data);
    }
}

fn tick_effect_data(data: &mut EffectData) {
    if data.duration > 0 {
        data.duration -= 1;
    }
    if let Some(nested) = &mut data.hidden_effect {
        tick_effect_data(nested);
    }
}

pub(crate) fn promote_hidden(
    hidden: &mut PlayerEffectsHidden,
    effect: MobEffect,
) -> Option<ActiveEffect> {
    let data = hidden.0.remove(&effect)?;
    if let Some(nested) = data.hidden_effect.clone() {
        hidden.0.insert(effect, *nested);
    }
    Some(active_from_data(effect, &data))
}
