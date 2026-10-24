use crate::identity;
use crate::packet_io::{ByteStream, PacketIo, decode_body};
use crate::{RawConnection, SwitchMode};
use anyhow::bail;
use mcrs_minecraft_core::{ResourceLocation, VERSION};
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::common::Brand;
use mcrs_minecraft_protocol::packets::common::serverbound::{
    ClientInformation, KeepAlive, ModList, Payload, PropertyMap,
};
use mcrs_minecraft_protocol::packets::configuration::clientbound::{
    ClientboundDisconnect, ClientboundFinishConfiguration, ClientboundKeepAlive,
    ClientboundRegistryData, ClientboundSelectKnownPacks, ClientboundUpdateTags, RegistryTags,
};
use mcrs_minecraft_protocol::packets::configuration::serverbound::{
    ServerboundClientInformation, ServerboundCustomPayload, ServerboundFinishConfiguration,
    ServerboundKeepAlive, ServerboundSelectKnownPacks,
};
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::login::clientbound::{
    ClientboundLoginDisconnect, ClientboundLoginFinished, ClientboundLoginFinishedWithOptions,
    LoginCompression, OptionFlags,
};
use mcrs_minecraft_protocol::packets::login::serverbound::{
    ServerboundHello, ServerboundLoginAcknowledged,
};
use mcrs_minecraft_protocol::resource_pack::KnownPack;
use mcrs_minecraft_protocol::setting::{ChatMode, DisplayedSkinParts, MainArm, ParticleStatus};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, CompressionThreshold, Packet, VarInt, WritePacket};
use mcrs_minecraft_registries::{Received, ReceivedTags};
use std::borrow::Cow;
use std::net::SocketAddr;
use tracing::info;

#[derive(Clone, Debug)]
pub struct Join {
    /// The host and port the handshake names, which a proxy can route by.
    pub server_address: String,
    pub server_port: u16,
    /// The peer of the stream, which the returned connection carries for log lines.
    pub remote_addr: SocketAddr,
    pub username: String,
    pub profile_id: Uuid,
    /// Sets only when the login switches compression on: the configuration that follows reads
    /// the packet ids and the vanilla pack version of this client's own version.
    pub protocol_version: i32,
    pub view_distance: u8,
    /// The client holds the vanilla pack of its own version, so it lets the server leave that
    /// pack's entries out.
    pub holds_vanilla_pack: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct ServerProfile {
    pub id: Uuid,
    pub username: String,
}

/// A connection whose first configuration the server ended: answered by [`join`], so in the game
/// state, and unanswered by [`configure`]. The packets the server sent after the end of the
/// configuration are buffered in it.
pub struct Joined {
    pub connection: RawConnection,
    pub profile: ServerProfile,
    pub configuration: Configuration,
}

/// Runs the handshake, the login and the first configuration in offline mode, and answers the
/// end of the configuration, so the connection is in the game state.
///
/// Must run inside a Tokio runtime on native targets, because the connection spawns its reader
/// and writer tasks there.
pub async fn join<S: ByteStream>(io: PacketIo<S>, join: Join) -> anyhow::Result<Joined> {
    let mut joined = configure(io, join).await?;
    joined
        .connection
        .write_packet_fallible(&ServerboundFinishConfiguration)?;
    joined.connection.flush()?;
    Ok(joined)
}

/// As [`join`], but leaves the end of the configuration unanswered, so a caller can check the
/// registries first and refuse them while the connection is still in configuration.
pub async fn configure<S: ByteStream>(mut io: PacketIo<S>, join: Join) -> anyhow::Result<Joined> {
    let (profile, switch_mode) = log_in(&mut io, &join).await?;
    write_first_configuration_packets(&mut io, join.view_distance);
    io.flush().await?;

    let mut collected = Configuration::default();
    let configuration = loop {
        let (id, body) = io.recv_frame().await?;
        let finished = collected.read(id, &body, join.holds_vanilla_pack, &mut io)?;
        io.flush().await?;
        if let Some(configuration) = finished {
            break configuration;
        }
    };
    info!(
        "configured by {}:{} as {}",
        join.server_address, join.server_port, profile.username
    );
    Ok(Joined {
        connection: io.into_raw_connection(join.remote_addr, switch_mode),
        profile,
        configuration,
    })
}

async fn log_in<S: ByteStream>(
    io: &mut PacketIo<S>,
    join: &Join,
) -> anyhow::Result<(ServerProfile, SwitchMode)> {
    let Some(switch_mode) = SwitchMode::of(join.protocol_version) else {
        bail!(
            "no login switch mode is known for protocol version {}",
            join.protocol_version
        );
    };
    io.send_packet(&ServerboundHandshake {
        protocol_version: VarInt(join.protocol_version),
        server_address: Bounded(join.server_address.as_str()),
        server_port: join.server_port,
        intent: Intent::Login,
    })
    .await?;
    io.send_packet(&ServerboundHello {
        username: Bounded(join.username.as_str()),
        profile_id: join.profile_id,
    })
    .await?;

    // A server that never announced a threshold still can ask for compression.
    let mut stored_threshold = CompressionThreshold(0);
    let (profile, options) = loop {
        let (id, body) = io.recv_frame().await?;
        if id == ClientboundLoginDisconnect::ID {
            let packet = decode_body::<ClientboundLoginDisconnect>(&body)?;
            bail!("server refused the login: {}", packet.reason.0);
        } else if id == LoginCompression::ID {
            let threshold =
                CompressionThreshold(decode_body::<LoginCompression>(&body)?.threshold.0);
            match switch_mode {
                SwitchMode::Immediate => io.set_compression(threshold),
                SwitchMode::Acknowledged => stored_threshold = threshold,
            }
        } else if id == ClientboundLoginFinished::ID {
            let (profile, options) = match switch_mode {
                SwitchMode::Immediate => (
                    decode_body::<ClientboundLoginFinished>(&body)?.profile,
                    OptionFlags::new(),
                ),
                SwitchMode::Acknowledged => {
                    let packet = decode_body::<ClientboundLoginFinishedWithOptions>(&body)?;
                    (packet.profile, packet.options)
                }
            };
            break (
                ServerProfile {
                    id: profile.id,
                    username: profile.username.0.to_owned(),
                },
                options,
            );
        } else {
            bail!("unexpected login packet {id:#04x}; only offline login is supported");
        }
    };

    if options.cipher() {
        bail!("the server asked for encryption; only offline login is supported");
    }
    let compression = options.compression().then_some(stored_threshold);
    if let Some(threshold) = compression {
        io.set_inbound_compression(threshold);
    }
    io.send_packet(&ServerboundLoginAcknowledged).await?;
    if let Some(threshold) = compression {
        io.set_outbound_compression(threshold);
    }
    Ok((profile, switch_mode))
}

pub(crate) fn write_first_configuration_packets(out: &mut impl WritePacket, view_distance: u8) {
    out.write_packet(&ServerboundCustomPayload::from(Payload::Brand(Brand {
        brand: identity::BRAND,
    })));
    out.write_packet(&ServerboundCustomPayload::from(Payload::ModList(ModList(
        vec![(
            identity::MOD_ENTRY.into(),
            PropertyMap(vec![(
                identity::COMMIT_PROPERTY.into(),
                identity::COMMIT_HASH,
            )]),
        )],
    ))));
    out.write_packet(&ServerboundClientInformation(ClientInformation {
        locale: "en_us",
        view_distance,
        chat_mode: ChatMode::Enabled,
        chat_colors: true,
        displayed_skin_parts: DisplayedSkinParts::from_bits(0x7f),
        main_arm: MainArm::Right,
        enable_text_filtering: false,
        allow_server_listings: true,
        particle_status: ParticleStatus::All,
    }));
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Configuration {
    pub registries: Received,
    /// The client accepted the vanilla pack, so the server may leave its entries' data out.
    pub vanilla_known: bool,
}

impl Configuration {
    /// Reads one configuration packet and writes its answer to `out`. Returns the whole
    /// configuration when the server ends it, and skips a packet it does not know.
    pub fn read(
        &mut self,
        id: i32,
        body: &[u8],
        holds_vanilla_pack: bool,
        out: &mut impl WritePacket,
    ) -> anyhow::Result<Option<Configuration>> {
        if id == ClientboundSelectKnownPacks::ID {
            let offer = decode_body::<ClientboundSelectKnownPacks>(body)?;
            let vanilla = KnownPack {
                namespace: "minecraft",
                id: "core",
                version: VERSION.id.as_str(),
            };
            let known_packs = if holds_vanilla_pack && offer.known_packs.contains(&vanilla) {
                vec![vanilla]
            } else {
                Vec::new()
            };
            self.vanilla_known = !known_packs.is_empty();
            out.write_packet_fallible(&ServerboundSelectKnownPacks { known_packs })?;
        } else if id == ClientboundRegistryData::ID {
            collect_registry(&mut self.registries, decode_body(body)?);
        } else if id == ClientboundUpdateTags::ID {
            let update = decode_body::<ClientboundUpdateTags>(body)?;
            self.registries
                .replace_tags(received_tags(update.registries));
        } else if id == ClientboundKeepAlive::ID {
            let keep_alive = decode_body::<ClientboundKeepAlive>(body)?;
            out.write_packet_fallible(&ServerboundKeepAlive(KeepAlive {
                payload: keep_alive.0.payload,
            }))?;
        } else if id == ClientboundDisconnect::ID {
            let disconnect = decode_body::<ClientboundDisconnect>(body)?;
            bail!(
                "the server disconnected you: {}",
                disconnect.reason.to_legacy_lossy()
            );
        } else if id == ClientboundFinishConfiguration::ID {
            decode_body::<ClientboundFinishConfiguration>(body)?;
            return Ok(Some(std::mem::take(self)));
        }
        Ok(None)
    }
}

fn collect_registry(received: &mut Received, data: ClientboundRegistryData<'_>) {
    let registry: ResourceLocation = data.registry.into();
    let entries = data
        .entries
        .into_iter()
        .map(|entry| (entry.id.into(), entry.data.map(Cow::into_owned)));
    match received
        .registries
        .iter_mut()
        .find(|(collected, _)| *collected == registry)
    {
        Some((_, collected)) => collected.extend(entries),
        None => received.registries.push((registry, entries.collect())),
    }
}

pub(crate) fn received_tags(update: Vec<RegistryTags<'_>>) -> ReceivedTags {
    update
        .into_iter()
        .map(|registry| {
            let tags = registry
                .tags
                .into_iter()
                .map(|group| {
                    let members = group.entries.into_iter().map(|member| member.0).collect();
                    (group.name.into(), members)
                })
                .collect();
            (registry.registry.into(), tags)
        })
        .collect()
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use mcrs_minecraft_protocol::decode::PacketDecoder;
    use mcrs_minecraft_protocol::packets::status::clientbound::StatusResponse;
    use mcrs_minecraft_protocol::profile::GameProfile;
    use mcrs_minecraft_protocol::{Decode, PacketEncoder};
    use std::time::Duration;

    #[test]
    fn the_client_sends_brand_then_mod_list_then_client_information() {
        let mut encoder = PacketEncoder::new();
        write_first_configuration_packets(&mut encoder, 11);
        let mut decoder = PacketDecoder::new();
        decoder.queue_bytes(encoder.take());
        let frames: Vec<_> = std::iter::from_fn(|| decoder.try_next_packet().unwrap()).collect();

        let ids: Vec<i32> = frames.iter().map(|frame| frame.id).collect();
        assert_eq!(
            ids,
            [
                ServerboundCustomPayload::ID,
                ServerboundCustomPayload::ID,
                ServerboundClientInformation::ID
            ]
        );

        let Payload::Brand(brand) = Payload::decode(&mut &frames[0].body[..]).unwrap() else {
            panic!("the first payload is not a brand");
        };
        assert_eq!(brand.brand, identity::BRAND);

        let Payload::ModList(ModList(entries)) = Payload::decode(&mut &frames[1].body[..]).unwrap()
        else {
            panic!("the second payload is not a mod list");
        };
        assert_eq!(entries.len(), 1);
        let (id, properties) = &entries[0];
        assert_eq!(*id, identity::MOD_ENTRY);
        assert_eq!(properties.0.len(), 1);
        assert_eq!(properties.0[0].0, identity::COMMIT_PROPERTY);
        assert_eq!(properties.0[0].1, identity::COMMIT_HASH);

        let information = ClientInformation::decode(&mut &frames[2].body[..]).unwrap();
        assert_eq!(information.view_distance, 11);
    }

    #[test]
    fn an_acknowledged_switch_login_compresses_each_direction_at_its_own_boundary() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let mut io = PacketIo::new(listener.accept().await.unwrap().0);
                let handshake = io.recv_packet::<ServerboundHandshake>().await.unwrap();
                assert_eq!(
                    handshake.protocol_version.0,
                    crate::PROTOCOL_26_4_SNAPSHOT_5
                );
                io.recv_frame().await.unwrap();
                io.send_packet(&LoginCompression {
                    threshold: VarInt(256),
                })
                .await
                .unwrap();
                io.send_packet(&ClientboundLoginFinishedWithOptions {
                    profile: GameProfile {
                        id: Uuid::nil(),
                        username: Bounded("Steve"),
                        properties: Default::default(),
                    },
                    session_id: Uuid::nil(),
                    options: OptionFlags::new().with_compression(true),
                })
                .await
                .unwrap();
                let (id, _) = io.recv_frame().await.unwrap();
                assert_eq!(
                    id,
                    ServerboundLoginAcknowledged::ID,
                    "an uncompressed acknowledgement"
                );
                io.set_compression(CompressionThreshold(256));
                io.send_packet(&ClientboundFinishConfiguration)
                    .await
                    .unwrap();
                loop {
                    let (id, _) = io.recv_frame().await.unwrap();
                    if id == ServerboundFinishConfiguration::ID {
                        break io;
                    }
                }
            });

            let io = PacketIo::connect(address).await.unwrap();
            let Joined {
                mut connection,
                profile,
                ..
            } = join(
                io,
                Join {
                    server_address: "127.0.0.1".to_owned(),
                    server_port: address.port(),
                    remote_addr: address,
                    username: "Steve".to_owned(),
                    profile_id: Uuid::nil(),
                    protocol_version: crate::PROTOCOL_26_4_SNAPSHOT_5,
                    view_distance: 8,
                    holds_vanilla_pack: false,
                },
            )
            .await
            .unwrap();
            assert_eq!(profile.username, "Steve");
            let mut server = server.await.unwrap();

            let json = "a".repeat(1000);
            let large = StatusResponse { json: &json };
            connection.append(&large).unwrap();
            connection.flush().unwrap();
            let (id, body) = server.recv_frame().await.unwrap();
            assert_eq!((id, body.len()), (StatusResponse::ID, 1002));

            server.send_packet(&large).await.unwrap();
            let received = tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    if let Some(packet) = connection.try_recv().unwrap() {
                        return packet;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(
                (received.id, received.payload.len()),
                (StatusResponse::ID, 1002)
            );
        });
    }
}
