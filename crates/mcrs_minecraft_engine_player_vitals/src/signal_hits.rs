use core::f64::consts::PI;

use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::query::With;
use bevy_ecs::system::{Query, Res};
use mcrs_minecraft_damage::{DamageSource, DamageTags, DamageTypeRules, HitOutcome};
use mcrs_minecraft_engine::bus::{self, OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine::player_signal::PlayerSignal;
use mcrs_minecraft_engine_damage::Damaged;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundHurtAnimation;
use mcrs_minecraft_replication::MobSignal;

pub(crate) fn signal_player_hits(
    mut damaged: MessageReader<Damaged>,
    players: Query<(&Transform, &Owner), With<Player>>,
    types: Res<DamageTypeRules>,
    mut signals: MessageWriter<PlayerSignal>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    for struck in damaged.read() {
        let Ok((transform, &Owner(session))) = players.get(struck.target) else {
            continue;
        };
        if struck.report.outcome == HitOutcome::Full {
            signals.write(PlayerSignal::new(
                struck.target,
                MobSignal::Hurt {
                    damage_type: struck.source.damage_type,
                },
            ));
            if !types
                .tags(struck.source.damage_type)
                .contains(DamageTags::NO_KNOCKBACK)
            {
                writer.write(bus::to(
                    session,
                    PacketPayload::HurtAnimation(ClientboundHurtAnimation {
                        entity_id: VarInt(player_network_id(session)),
                        yaw: hurt_yaw(transform, &struck.source),
                    }),
                ));
            }
        }
        if struck.report.died {
            signals.write(PlayerSignal::new(struck.target, MobSignal::Died));
        }
    }
}

fn hurt_yaw(target: &Transform, source: &DamageSource) -> f32 {
    let (dx, dz) = source.position.map_or((0.0, 0.0), |position| {
        (
            position.x - target.translation.x,
            position.z - target.translation.z,
        )
    });
    (libm::atan2(dz, dx) * 180.0 / PI - f64::from(target.rotation.yaw())) as f32
}
