use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_engine_item::{Slots, StackCommandsExt};
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_game_rule::GameRule;

use crate::events::DeathEvent;
use crate::position::CombatPosition;

/// Event emitted when an entity's inventory should be dropped at the death position (#116).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct DropInventoryOnDeathEvent {
    pub entity: Entity,
    pub death_position: DVec3,
}

/// Handles death events, scattering the entity's inventory at death unless `keep_inventory` is enabled.
pub fn handle_death_drops(
    mut death_events: MessageReader<DeathEvent>,
    rules_opt: Option<Res<GameRules>>,
    positions: Query<&CombatPosition>,
    mut drop_events: MessageWriter<DropInventoryOnDeathEvent>,
) {
    let keep_inventory = rules_opt
        .map(|r| r.is_on(GameRule::KeepInventory))
        .unwrap_or(false);

    for ev in death_events.read() {
        if !keep_inventory {
            let pos = positions.get(ev.entity).map(|p| p.0).unwrap_or(DVec3::ZERO);
            drop_events.write(DropInventoryOnDeathEvent {
                entity: ev.entity,
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
        for stack in held.occupied() {
            commands.entity(stack).drop_at(drop.death_position);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, Update};

    #[test]
    fn test_death_drops_honors_keep_inventory() {
        let mut app = App::new();
        app.add_message::<DeathEvent>();
        app.add_message::<DropInventoryOnDeathEvent>();
        app.add_systems(Update, handle_death_drops);

        let mut rules = GameRules::default();
        rules.set(GameRule::KeepInventory, 1);
        app.insert_resource(rules);

        let entity = app
            .world_mut()
            .spawn(CombatPosition::from_coords(10.0, 64.0, 10.0))
            .id();

        let mut death_writer = app
            .world_mut()
            .resource_mut::<bevy_ecs::message::Messages<DeathEvent>>();
        death_writer.write(DeathEvent {
            entity,
            damage_type: crate::damage_types::GENERIC,
            source: mcrs_minecraft_damage::DamageSource::new(crate::damage_types::GENERIC),
        });

        app.update();

        // With keepInventory = 1, no drops should be emitted
        let drop_events = app
            .world()
            .resource::<bevy_ecs::message::Messages<DropInventoryOnDeathEvent>>();
        assert_eq!(drop_events.len(), 0);
    }
}
