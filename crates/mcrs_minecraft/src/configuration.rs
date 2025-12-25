use crate::dimension_type::DimensionType;
use crate::version::VERSION_ID;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{Changed, Commands, Entity, On, Query};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_network::{ConnectionState, InGameConnectionState, ServerSideConnection};
use mcrs_protocol::packets::configuration::clientbound::ClientboundSelectKnownPacks;
use mcrs_protocol::packets::configuration::serverbound::ServerboundFinishConfiguration;
use mcrs_protocol::packets::configuration::{
    ClientboundFinishConfiguration, ClientboundRegistryData,
};
use mcrs_protocol::registry::Entry;
use mcrs_protocol::resource_pack::KnownPack;
use mcrs_protocol::{Ident, WritePacket, ident, nbt};
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::str::FromStr;

pub(crate) struct ConfigurationStatePlugin;

impl Plugin for ConfigurationStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(bevy_app::FixedPreUpdate, on_configuration_enter);
        app.insert_resource(SyncedRegistries(init_synced_registries()));
        app.add_observer(on_configuration_ack);
    }
}

fn on_configuration_enter(
    mut query: Query<
        (Entity, &mut ServerSideConnection, &ConnectionState),
        Changed<ConnectionState>,
    >,
    res: Res<SyncedRegistries>,
) {
    for (entity, mut con, conn_state) in query.iter_mut() {
        if *conn_state != ConnectionState::Configuration {
            continue;
        }

        con.write_packet(&ClientboundSelectKnownPacks {
            known_packs: vec![KnownPack {
                namespace: "minecraft",
                id: "core",
                version: VERSION_ID,
            }],
        });

        let requeried_regs = vec!["variant", "damage_type"];
        for (registry_id, entries) in &res.0 {
            if !requeried_regs
                .iter()
                .any(|r| registry_id.path().contains(r))
            {
                continue;
            }

            let packet_entries = entries
                .iter()
                .map(|name| Entry {
                    id: Cow::from(name.as_str()).try_into().unwrap(),
                    data: None,
                })
                .collect::<Vec<_>>();

            let packet = ClientboundRegistryData {
                registry: Cow::from(registry_id.as_str()).try_into().unwrap(),
                entries: packet_entries,
            };
            println!("sending registry data {:?}", &packet);
            con.write_packet(&packet);
        }

        let dim_nbt = nbt::to_nbt_compound(&DimensionType {
            min_y: 0,
            height: 256,
            logical_height: 256,
            ambient_light: 1.0,
            ..Default::default()
        })
        .unwrap();
        con.write_packet(&ClientboundRegistryData {
            registry: ident!("minecraft:dimension_type").into(),
            entries: vec![Entry {
                id: ident!("minecraft:overworld").into(),
                data: Some(Cow::Owned(dim_nbt)),
            }],
        });

        con.write_packet(&ClientboundRegistryData {
            registry: ident!("minecraft:worldgen/biome").into(),
            entries: vec![Entry {
                id: ident!("minecraft:plains").into(),
                data: None,
            }],
        });

        con.write_packet(&ClientboundFinishConfiguration)
    }
}

fn on_configuration_ack(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(Entity, &mut ConnectionState)>,
    mut commands: Commands,
) {
    let Ok((entity, mut state)) = query.get_mut(event.entity) else {
        return;
    };
    if *state != ConnectionState::Configuration {
        return;
    }
    let Some(_) = event.decode::<ServerboundFinishConfiguration>() else {
        return;
    };
    *state = ConnectionState::Game;
    commands.entity(entity).insert(InGameConnectionState);
}

#[derive(Default, Resource)]
struct SyncedRegistries(Vec<(Ident<String>, Vec<Ident<String>>)>);

fn init_synced_registries() -> Vec<(Ident<String>, Vec<Ident<String>>)> {
    let synced_registries = include_str!("../../../assets/synced_registries.json");
    let json = serde_json::from_str::<Map<String, Value>>(synced_registries).unwrap();
    json.iter()
        .map(|(registry_id, registry)| {
            let registry = registry.as_object().unwrap();
            let entries = registry
                .iter()
                .map(|(name, _value)| Ident::from_str(name).unwrap())
                .collect::<Vec<_>>();
            let registry_id: Ident<String> = Ident::from_str(registry_id).unwrap();
            (registry_id, entries)
        })
        .collect::<Vec<_>>()
}
