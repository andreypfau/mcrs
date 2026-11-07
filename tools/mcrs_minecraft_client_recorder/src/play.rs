use glam::DVec3;
use mcrs_minecraft_client_ticks::Abilities;
use mcrs_minecraft_client_ticks::Attribute;
use mcrs_minecraft_client_ticks::BlockChange;
use mcrs_minecraft_client_ticks::Effect;
use mcrs_minecraft_client_ticks::Event;
use mcrs_minecraft_client_ticks::Keys;
use mcrs_minecraft_client_ticks::Modifier;
use mcrs_minecraft_client_ticks::Move;
use mcrs_minecraft_client_ticks::Rotation;
use mcrs_minecraft_client_ticks::Teleport;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_item::mob_effect_keys::MobEffect;
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::packets::table::game_clientbound;
use mcrs_minecraft_protocol::packets::table::game_serverbound;
use mcrs_minecraft_protocol_game::LpVec3;
use mcrs_minecraft_protocol_game::MoveFlags;
use mcrs_minecraft_protocol_game::Position;
use mcrs_minecraft_protocol_game::clientbound::AttributeOperation;
use mcrs_minecraft_protocol_game::clientbound::AttributeSnapshot;
use mcrs_minecraft_protocol_game::clientbound::ClientboundBlockUpdate;
use mcrs_minecraft_protocol_game::clientbound::ClientboundForgetLevelChunk;
use mcrs_minecraft_protocol_game::clientbound::ClientboundPlayerAbilities;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSectionBlocksUpdate;
use mcrs_minecraft_protocol_game::clientbound::ClientboundUpdateAttributes;
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::PlayerCommandAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundMovePlayerPos;
use mcrs_minecraft_protocol_game::serverbound::ServerboundMovePlayerPosRot;
use mcrs_minecraft_protocol_game::serverbound::ServerboundMovePlayerRot;
use mcrs_minecraft_protocol_game::serverbound::ServerboundMovePlayerStatusOnly;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerCommand;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerInput;

use crate::names::Names;

/// What a play packet means to the recording.
pub(crate) enum Play {
    /// `login`: the network id of the player.
    Login(i32),
    /// `player_loaded`: the client has loaded the world.
    Loaded,
    /// `ping` or `pong`: the id.
    Ping(i32),
    /// `bundle_delimiter`.
    Bundle,
    /// The events of the packet; none for a packet that the recording does not keep.
    Events(Vec<Event>),
}

/// The name of the packet `id` in a packet table; empty for an id past its end.
pub(crate) fn name_of(names: &'static [&'static str], id: i32) -> &'static str {
    usize::try_from(id)
        .ok()
        .and_then(|index| names.get(index))
        .copied()
        .unwrap_or_default()
}

/// Decodes a clientbound play packet. `player` is the network id of the player, once known.
pub(crate) fn decode_clientbound(
    names: &Names,
    player: Option<i32>,
    id: i32,
    mut body: &[u8],
) -> anyhow::Result<Play> {
    let r = &mut body;
    let event = match name_of(game_clientbound::NAMES, id) {
        "bundle_delimiter" => return Ok(Play::Bundle),
        "login" => return Ok(Play::Login(i32::decode(r)?)),
        "ping" => return Ok(Play::Ping(i32::decode(r)?)),
        "player_position" => Event::Teleport(Teleport {
            id: VarInt::decode(r)?.0,
            position: vec3(r)?,
            velocity: vec3(r)?,
            rotation: Rotation {
                yaw: f32::decode(r)?,
                pitch: f32::decode(r)?,
            },
            relative: u32::from_be_bytes(i32::decode(r)?.to_be_bytes()),
        }),
        "set_entity_motion" if is_player(r, player)? => {
            Event::Motion(LpVec3::decode(r)?.0.to_array())
        }
        "explode" => {
            let _center = vec3(r)?;
            let _radius = f32::decode(r)?;
            let _block_count = i32::decode(r)?;
            if !bool::decode(r)? {
                return Ok(Play::Events(Vec::new()));
            }
            Event::Explosion(vec3(r)?)
        }
        "player_abilities" => {
            let packet = ClientboundPlayerAbilities::decode(r)?;
            Event::Abilities(Abilities {
                flying: packet.flying,
                may_fly: packet.may_fly,
                flying_speed: packet.flying_speed,
            })
        }
        "update_attributes" => {
            let packet = ClientboundUpdateAttributes::decode(r)?;
            if Some(packet.entity_id.0) != player {
                return Ok(Play::Events(Vec::new()));
            }
            return Ok(Play::Events(
                packet.attributes.iter().map(attribute).collect(),
            ));
        }
        "update_mob_effect" if is_player(r, player)? => Event::Effect(Effect {
            name: effect_name(VarInt::decode(r)?.0),
            amplifier: u8::try_from(VarInt::decode(r)?.0)?,
        }),
        "remove_mob_effect" if is_player(r, player)? => {
            Event::RemoveEffect(effect_name(VarInt::decode(r)?.0))
        }
        "block_update" => {
            let packet = ClientboundBlockUpdate::decode(r)?;
            Event::Block(BlockChange {
                position: [packet.block_pos.x, packet.block_pos.y, packet.block_pos.z],
                state: names.block_state(packet.block_state_id.0),
            })
        }
        "section_blocks_update" => {
            let packet = ClientboundSectionBlocksUpdate::decode(r)?;
            let origin = *packet.chunk_pos * 16;
            let events = packet
                .blocks
                .iter()
                .map(|entry| {
                    Event::Block(BlockChange {
                        position: [
                            origin.x + i32::from(entry.off_x()),
                            origin.y + i32::from(entry.off_y()),
                            origin.z + i32::from(entry.off_z()),
                        ],
                        state: names.block_state(entry.block_state()),
                    })
                })
                .collect();
            return Ok(Play::Events(events));
        }
        "forget_level_chunk" => {
            let packet = ClientboundForgetLevelChunk::decode(r)?;
            Event::ChunkForgotten([packet.x, packet.z])
        }
        _ => return Ok(Play::Events(Vec::new())),
    };
    Ok(Play::Events(vec![event]))
}

/// Decodes a serverbound play packet.
pub(crate) fn decode_serverbound(id: i32, mut body: &[u8]) -> anyhow::Result<Play> {
    let r = &mut body;
    let event = match name_of(game_serverbound::NAMES, id) {
        "pong" => return Ok(Play::Ping(i32::decode(r)?)),
        "player_loaded" => return Ok(Play::Loaded),
        "player_input" => {
            let input = ServerboundPlayerInput::decode(r)?.input;
            Event::Input(Keys {
                forward: input.forward(),
                backward: input.backward(),
                left: input.left(),
                right: input.right(),
                jump: input.jump(),
                shift: input.shift(),
                sprint: input.sprint(),
            })
        }
        "player_command" => match ServerboundPlayerCommand::decode(r)?.action {
            PlayerCommandAction::StartSprinting => Event::Sprinting(true),
            PlayerCommandAction::StopSprinting => Event::Sprinting(false),
            _ => return Ok(Play::Events(Vec::new())),
        },
        "use_item" => Event::UseItem,
        "player_action"
            if ServerboundPlayerAction::decode(r)?.action == PlayerAction::ReleaseUseItem =>
        {
            Event::ReleaseUseItem
        }
        "move_player_pos" => {
            let packet = ServerboundMovePlayerPos::decode(r)?;
            step(Some(coordinates(packet.position)), None, packet.flags)
        }
        "move_player_pos_rot" => {
            let packet = ServerboundMovePlayerPosRot::decode(r)?;
            let look = rotation(packet.look);
            step(Some(coordinates(packet.position)), Some(look), packet.flags)
        }
        "move_player_rot" => {
            let packet = ServerboundMovePlayerRot::decode(r)?;
            step(None, Some(rotation(packet.look)), packet.flags)
        }
        "move_player_status_only" => step(
            None,
            None,
            ServerboundMovePlayerStatusOnly::decode(r)?.flags,
        ),
        "client_tick_end" => Event::TickEnd,
        _ => return Ok(Play::Events(Vec::new())),
    };
    Ok(Play::Events(vec![event]))
}

/// Reads the entity id at the start of a packet and tells whether it is the player's.
fn is_player(r: &mut &[u8], player: Option<i32>) -> anyhow::Result<bool> {
    Ok(Some(VarInt::decode(r)?.0) == player)
}

fn step(position: Option<[f64; 3]>, rotation: Option<Rotation>, flags: MoveFlags) -> Event {
    Event::Move(Move {
        position,
        rotation,
        on_ground: flags.on_ground(),
        horizontal_collision: flags.horizontal_collision(),
    })
}

fn rotation(look: Look) -> Rotation {
    Rotation {
        yaw: look.yaw,
        pitch: look.pitch,
    }
}

fn coordinates(position: Position) -> [f64; 3] {
    DVec3::from(position).to_array()
}

fn vec3(r: &mut &[u8]) -> anyhow::Result<[f64; 3]> {
    Ok([f64::decode(r)?, f64::decode(r)?, f64::decode(r)?])
}

fn attribute(snapshot: &AttributeSnapshot<'_>) -> Event {
    let modifiers = snapshot
        .modifiers
        .iter()
        .map(|modifier| Modifier {
            id: modifier.id.to_string(),
            amount: modifier.amount,
            operation: match modifier.operation {
                AttributeOperation::AddValue => 0,
                AttributeOperation::AddMultipliedBase => 1,
                AttributeOperation::AddMultipliedTotal => 2,
            },
        })
        .collect();
    Event::Attribute(Attribute {
        name: snapshot.attribute.as_static_str().to_owned(),
        base: snapshot.base,
        modifiers,
    })
}

fn effect_name(id: i32) -> String {
    u16::try_from(id)
        .ok()
        .and_then(MobEffect::from_protocol_id)
        .map_or_else(
            || format!("#{id}"),
            |effect| effect.as_static_str().to_owned(),
        )
}
