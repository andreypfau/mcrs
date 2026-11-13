use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_engine_item::{Slots, StackCommandsExt};
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_game_rule::GameRule;

use crate::events::DamageTarget;
use crate::events::DeathEvent;
use mcrs_minecraft_engine_world::entity::physics::Transform;

/// Event emitted when an entity's inventory should be dropped at the death position.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct DropInventoryOnDeathEvent {
    pub entity: Entity,
    pub death_position: DVec3,
}

/// Handles death events, scattering the entity's inventory at death unless `keep_inventory` is enabled.
pub fn handle_death_drops(
    mut death_events: MessageReader<DeathEvent>,
    rules_opt: Option<Res<GameRules>>,
    positions: Query<&Transform>,
    mut drop_events: MessageWriter<DropInventoryOnDeathEvent>,
) {
    let keep_inventory = rules_opt
        .map(|r| r.is_on(GameRule::KeepInventory))
        .unwrap_or(false);

    for ev in death_events.read() {
        let DamageTarget::Player(entity) = ev.target else {
            continue;
        };
        if !keep_inventory {
            let pos = positions
                .get(entity)
                .map(|transform| transform.translation)
                .unwrap_or(DVec3::ZERO);
            drop_events.write(DropInventoryOnDeathEvent {
                entity,
                death_position: pos,
            });
        }
    }
}

/// Puts the stacks of a dead entity's slots into the world, which clears them from the holder
/// through the placement hooks.
pub fn drop_dead_inventories(
    mut drops: MessageReader<DropInventoryOnDeathEvent>,
    slots: Query<&Slots>,
    mut commands: Commands,
) {
    for drop in drops.read() {
        let Ok(held) = slots.get(drop.entity) else {
            continue;
        };
        for (index, stack) in held.occupied().enumerate() {
            commands
                .entity(stack)
                .drop_at(drop.death_position + scatter_offset(index));
        }
    }
}

fn scatter_offset(index: usize) -> DVec3 {
    let angle = index as f64 * 2.399_963_229_728_653 + 0.5;
    let radius = 0.15 + (index % 3) as f64 * 0.05;
    DVec3::new(angle.cos() * radius, 0.1, angle.sin() * radius)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, Update};

    fn drops_for(keep_inventory: bool) -> usize {
        let mut app = App::new();
        app.add_message::<DeathEvent>();
        app.add_message::<DropInventoryOnDeathEvent>();
        app.add_systems(Update, handle_death_drops);
        let mut rules = GameRules::default();
        if keep_inventory {
            rules.set(GameRule::KeepInventory, 1);
        }
        app.insert_resource(rules);
        let entity = app
            .world_mut()
            .spawn(Transform::from_xyz(10.0, 64.0, 10.0))
            .id();
        app.world_mut()
            .resource_mut::<bevy_ecs::message::Messages<DeathEvent>>()
            .write(DeathEvent {
                target: DamageTarget::Player(entity),
                source: mcrs_minecraft_damage::DamageSource::new(0),
            });
        app.update();
        app.world()
            .resource::<bevy_ecs::message::Messages<DropInventoryOnDeathEvent>>()
            .len()
    }

    #[test]
    fn test_death_drops_honors_keep_inventory() {
        assert_eq!(drops_for(false), 1);
        assert_eq!(drops_for(true), 0);
    }
}
