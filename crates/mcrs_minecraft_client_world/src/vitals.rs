use bevy::prelude::*;
use mcrs_minecraft_entity::attribute_instance::fold_attribute;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_item::ItemHoverMap;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundHurtAnimation, ClientboundPlayerCombatKill, ClientboundRemoveMobEffect,
    ClientboundRespawn, ClientboundSetEntityData, ClientboundSetExperience, ClientboundSetHealth,
    ClientboundUpdateAttributes, ClientboundUpdateMobEffect,
};
use mcrs_minecraft_protocol_game::entity::MetaDataValue;
use mcrs_minecraft_protocol_game::entity_data_index;
use mcrs_minecraft_protocol_game::text::Text;

use crate::joined_game::JoinedGame;
use crate::player::Player;

pub const MAX_HEALTH: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisibleEffect {
    pub effect: u8,
    pub amplifier: u8,
    pub duration: i32,
    pub ambient: bool,
    pub show_particles: bool,
    pub show_icon: bool,
}

#[derive(Component, Clone, Debug, PartialEq)]
pub struct LocalVitals {
    pub health: f32,
    pub max_health: f32,
    pub absorption: f32,
    pub armor: f32,
    pub food: i32,
    pub saturation: f32,
    pub experience_level: i32,
    pub experience_progress: f32,
    pub total_experience: i32,
    pub effects: Vec<VisibleEffect>,
    pub air: i32,
    pub max_air: i32,
    pub frozen_ticks: i32,
    pub hurt_time: i32,
    pub on_fire: bool,
    pub dead: bool,
    pub death_message: Option<Text>,
}

impl Default for LocalVitals {
    fn default() -> Self {
        Self {
            health: MAX_HEALTH,
            max_health: MAX_HEALTH,
            absorption: 0.0,
            armor: 0.0,
            food: 20,
            saturation: 5.0,
            experience_level: 0,
            experience_progress: 0.0,
            total_experience: 0,
            effects: Vec::new(),
            air: 300,
            max_air: 300,
            frozen_ticks: 0,
            hurt_time: 0,
            on_fire: false,
            dead: false,
            death_message: None,
        }
    }
}

impl LocalVitals {
    #[must_use]
    pub fn has(&self, effect: u8) -> bool {
        self.effects.iter().any(|active| active.effect == effect)
    }
}

pub struct VitalsPlugin;

impl Plugin for VitalsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_health)
            .add_observer(on_experience)
            .add_observer(on_attributes)
            .add_observer(on_effect)
            .add_observer(on_effect_removed)
            .add_observer(on_entity_data)
            .add_observer(on_hurt_animation)
            .add_observer(on_death)
            .add_observer(on_respawn)
            .add_systems(FixedUpdate, (tick_hurt, tick_effects));
    }
}

fn tick_effects(mut players: Query<&mut LocalVitals, With<Player>>) {
    for mut local in &mut players {
        for effect in &mut local.effects {
            if effect.duration > 0 {
                effect.duration -= 1;
            }
        }
        local.effects.retain(|effect| effect.duration != 0);
    }
}

fn tick_hurt(mut players: Query<&mut LocalVitals, With<Player>>) {
    for mut local in &mut players {
        local.hurt_time = local.hurt_time.saturating_sub(1);
    }
}

fn on_hurt_animation(
    event: On<Received<ClientboundHurtAnimation>>,
    connections: Query<&JoinedGame>,
    mut players: Query<&mut LocalVitals, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if !own_event(packet.entity_id.0, event.entity, &connections) {
        return;
    }
    if let Ok(mut local) = players.single_mut() {
        local.hurt_time = 10;
    }
}

fn on_health(
    event: On<Received<ClientboundSetHealth>>,
    mut vitals: Query<&mut LocalVitals, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Ok(mut local) = vitals.single_mut() else {
        return;
    };
    local.health = packet.health.max(0.0);
    local.food = packet.food.0.clamp(0, 20);
    local.saturation = packet.saturation.max(0.0);
    local.dead = local.health <= 0.0;
}

fn on_experience(
    event: On<Received<ClientboundSetExperience>>,
    mut vitals: Query<&mut LocalVitals, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Ok(mut local) = vitals.single_mut() else {
        return;
    };
    local.experience_level = packet.experience_level.0;
    local.experience_progress = packet.experience_progress.clamp(0.0, 1.0);
    local.total_experience = packet.total_experience.0;
}

fn on_attributes(
    event: On<Received<ClientboundUpdateAttributes<'static>>>,
    connections: Query<&JoinedGame>,
    mut players: Query<&mut LocalVitals, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if !own_event(packet.entity_id.0, event.entity, &connections) {
        return;
    }
    let Ok(mut local) = players.single_mut() else {
        return;
    };
    for snapshot in &packet.attributes {
        let value = fold_attribute(
            snapshot.base,
            snapshot
                .modifiers
                .iter()
                .map(|modifier| (modifier.operation.into(), modifier.amount)),
        );
        if snapshot.attribute == Attribute::MaxHealth {
            local.max_health = (value as f32).max(1.0);
            local.health = local.health.clamp(0.0, local.max_health);
        } else if snapshot.attribute == Attribute::Armor {
            local.armor = (value as f32).max(0.0);
        }
    }
}

fn own_event(packet_entity: i32, connection: Entity, connections: &Query<&JoinedGame>) -> bool {
    connections
        .get(connection)
        .is_ok_and(|joined| joined.player_id == packet_entity)
}

fn on_effect(
    event: On<Received<ClientboundUpdateMobEffect>>,
    connections: Query<&JoinedGame>,
    mut players: Query<&mut LocalVitals, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if !own_event(packet.entity_id.0, event.entity, &connections) {
        return;
    }
    let Ok(mut local) = players.single_mut() else {
        return;
    };
    let effect = VisibleEffect {
        effect: u8::try_from(packet.effect.0).unwrap_or(u8::MAX),
        amplifier: u8::try_from(packet.amplifier.0).unwrap_or(u8::MAX),
        duration: packet.duration.0,
        ambient: packet.flags & 0x01 != 0,
        show_particles: packet.flags & 0x02 != 0,
        show_icon: packet.flags & 0x04 != 0,
    };
    if let Some(existing) = local
        .effects
        .iter_mut()
        .find(|existing| existing.effect == effect.effect)
    {
        *existing = effect;
    } else {
        local.effects.push(effect);
    }
}

fn on_effect_removed(
    event: On<Received<ClientboundRemoveMobEffect>>,
    connections: Query<&JoinedGame>,
    mut players: Query<&mut LocalVitals, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if !own_event(packet.entity_id.0, event.entity, &connections) {
        return;
    }
    let Ok(mut local) = players.single_mut() else {
        return;
    };
    let effect = u8::try_from(packet.effect.0).unwrap_or(u8::MAX);
    local.effects.retain(|active| active.effect != effect);
}

fn on_entity_data(
    event: On<Received<ClientboundSetEntityData<'static>>>,
    connections: Query<&JoinedGame>,
    mut players: Query<&mut LocalVitals, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if !own_event(packet.entity_id.0, event.entity, &connections) {
        return;
    }
    let Ok(mut local) = players.single_mut() else {
        return;
    };
    for entry in &packet.metadata.0 {
        match (entry.index, &entry.value) {
            (entity_data_index::SHARED_FLAGS, MetaDataValue::Byte(flags)) => {
                local.on_fire = flags & 0x01 != 0;
            }
            (entity_data_index::AIR_SUPPLY, MetaDataValue::VarInt(air)) => {
                local.air = air.0.clamp(0, local.max_air);
            }
            (entity_data_index::TICKS_FROZEN, MetaDataValue::VarInt(frozen)) => {
                local.frozen_ticks = frozen.0;
            }
            _ => {}
        }
    }
}

fn on_death(
    event: On<Received<ClientboundPlayerCombatKill<'static>>>,
    connections: Query<&JoinedGame>,
    mut players: Query<&mut LocalVitals, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if !own_event(packet.player_id.0, event.entity, &connections) {
        return;
    }
    if let Ok(mut local) = players.single_mut() {
        local.dead = true;
        local.health = 0.0;
        local.death_message = packet
            .message
            .to_text()
            .ok()
            .and_then(|message| message.map(&ItemHoverMap).ok());
    }
}

fn on_respawn(
    _: On<Received<ClientboundRespawn>>,
    mut players: Query<&mut LocalVitals, With<Player>>,
) {
    if let Ok(mut local) = players.single_mut() {
        *local = LocalVitals::default();
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::{App, Update};
    use mcrs_minecraft_network::ConnectionState;
    use mcrs_minecraft_network::metrics::PacketDispatcher;
    use mcrs_minecraft_protocol::{Encode, Packet, VarInt};

    use super::*;

    const PLAYER_ID: i32 = 7;

    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(VitalsPlugin);
        let connection = app
            .world_mut()
            .spawn((
                ConnectionState::Game,
                JoinedGame {
                    player_id: PLAYER_ID,
                    dimensions: Vec::new(),
                },
            ))
            .id();
        app.world_mut().spawn((Player, LocalVitals::default()));
        (app, connection)
    }

    fn receive<P: Packet + Encode>(app: &mut App, connection: Entity, packet: P) {
        let mut data = Vec::new();
        packet.encode(&mut data).unwrap();
        let trigger = PacketDispatcher::clientbound()
            .with_play(crate::game_packets::GAME_CLIENTBOUND)
            .dispatch(P::STATE, P::ID, &data)
            .unwrap()
            .unwrap();
        trigger.raise(&mut app.world_mut().commands(), connection, data.into());
        app.world_mut().flush();
    }

    fn effect(duration: i32) -> VisibleEffect {
        VisibleEffect {
            effect: 1,
            amplifier: 0,
            duration,
            ambient: false,
            show_particles: true,
            show_icon: true,
        }
    }

    #[test]
    fn dmg_net_004_health_above_the_default_maximum_is_not_clamped() {
        let (mut app, connection) = app();
        receive(
            &mut app,
            connection,
            ClientboundSetHealth {
                health: 30.0,
                food: VarInt(18),
                saturation: 2.5,
            },
        );
        let local = app
            .world_mut()
            .query_filtered::<&LocalVitals, With<Player>>()
            .single(app.world())
            .unwrap();
        assert_eq!(local.health, 30.0);
        assert_eq!(local.food, 18);
        assert_eq!(local.saturation, 2.5);
    }

    #[test]
    fn dmg_net_004_experience_reads_progress_level_and_total_in_order() {
        let (mut app, connection) = app();
        receive(
            &mut app,
            connection,
            ClientboundSetExperience {
                experience_progress: 0.5,
                experience_level: VarInt(3),
                total_experience: VarInt(25),
            },
        );
        let local = app
            .world_mut()
            .query_filtered::<&LocalVitals, With<Player>>()
            .single(app.world())
            .unwrap();
        assert_eq!(local.experience_progress, 0.5);
        assert_eq!(local.experience_level, 3);
        assert_eq!(local.total_experience, 25);
    }

    #[test]
    fn dmg_net_003_client_effect_durations_tick_and_infinite_effects_stay() {
        let mut app = App::new();
        app.add_systems(Update, tick_effects);
        let player = app
            .world_mut()
            .spawn((
                Player,
                LocalVitals {
                    effects: vec![effect(2), effect(-1)],
                    ..LocalVitals::default()
                },
            ))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<LocalVitals>(player).unwrap().effects[0].duration,
            1
        );
        app.update();
        let effects = &app.world().get::<LocalVitals>(player).unwrap().effects;
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].duration, -1);
    }
}
