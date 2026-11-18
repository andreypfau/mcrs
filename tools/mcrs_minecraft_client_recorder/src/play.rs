use glam::DVec3;
use mcrs_minecraft_client_ticks::Abilities;
use mcrs_minecraft_client_ticks::Attribute;
use mcrs_minecraft_client_ticks::BlockChange;
use mcrs_minecraft_client_ticks::ChangedSlot;
use mcrs_minecraft_client_ticks::Click;
use mcrs_minecraft_client_ticks::Content;
use mcrs_minecraft_client_ticks::DataEntity;
use mcrs_minecraft_client_ticks::Effect;
use mcrs_minecraft_client_ticks::EntityData;
use mcrs_minecraft_client_ticks::Event;
use mcrs_minecraft_client_ticks::Keys;
use mcrs_minecraft_client_ticks::Modifier;
use mcrs_minecraft_client_ticks::Move;
use mcrs_minecraft_client_ticks::Rotation;
use mcrs_minecraft_client_ticks::SlotChange;
use mcrs_minecraft_client_ticks::Teleport;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_entity::mob_effect_keys::MobEffect;
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::packets::table::game_clientbound;
use mcrs_minecraft_protocol::packets::table::game_serverbound;
use mcrs_minecraft_protocol_game::LpVec3;
use mcrs_minecraft_protocol_game::MoveFlags;
use mcrs_minecraft_protocol_game::Position;
use mcrs_minecraft_protocol_game::clientbound::AttributeSnapshot;
use mcrs_minecraft_protocol_game::clientbound::ClientboundBlockUpdate;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetContent;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetSlot;
use mcrs_minecraft_protocol_game::clientbound::ClientboundForgetLevelChunk;
use mcrs_minecraft_protocol_game::clientbound::ClientboundPlayerAbilities;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSectionBlocksUpdate;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetCursorItem;
use mcrs_minecraft_protocol_game::clientbound::ClientboundUpdateAttributes;
use mcrs_minecraft_protocol_game::entity::MetaDataValue;
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::PlayerCommandAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClick;
use mcrs_minecraft_protocol_game::serverbound::ServerboundInteract;
use mcrs_minecraft_protocol_game::serverbound::ServerboundMovePlayerPos;
use mcrs_minecraft_protocol_game::serverbound::ServerboundMovePlayerPosRot;
use mcrs_minecraft_protocol_game::serverbound::ServerboundMovePlayerRot;
use mcrs_minecraft_protocol_game::serverbound::ServerboundMovePlayerStatusOnly;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerCommand;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerInput;

use crate::names::Names;
use crate::stack_text::hashed_stack_text;
use crate::stack_text::sent_stack_text;

pub(crate) enum Play {
    Login(i32),
    Loaded,
    Ping(i32),
    Bundle,
    Events(Vec<Event>),
}

pub(crate) fn name_of(names: &'static [&'static str], id: i32) -> &'static str {
    usize::try_from(id)
        .ok()
        .and_then(|index| names.get(index))
        .copied()
        .unwrap_or_default()
}

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
            let events: Vec<Event> = packet
                .attributes
                .iter()
                .map(attribute)
                .collect::<anyhow::Result<_>>()?;
            return Ok(Play::Events(events));
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
        "set_entity_data" => {
            let id = VarInt::decode(r)?.0;
            let entity = if Some(id) == player {
                DataEntity::Own
            } else {
                DataEntity::Other(id)
            };
            return Ok(Play::Events(entity_data(entity, r)));
        }
        name => {
            return Ok(Play::Events(
                container_event(name, r)?.into_iter().collect(),
            ));
        }
    };
    Ok(Play::Events(vec![event]))
}

fn container_event(name: &str, r: &mut &[u8]) -> anyhow::Result<Option<Event>> {
    Ok(Some(match name {
        "container_set_slot" => {
            let packet = ClientboundContainerSetSlot::decode(r)?;
            Event::Slot(SlotChange {
                container: packet.container_id.0,
                state: packet.state_seqno.0,
                slot: packet.slot,
                stack: sent_stack_text(&packet.item),
            })
        }
        "container_set_content" => {
            let packet = ClientboundContainerSetContent::decode(r)?;
            Event::Content(Content {
                container: packet.container_id.0,
                state: packet.state_seqno.0,
                cursor: sent_stack_text(&packet.carried_item),
                slots: packet.slot_data.iter().map(sent_stack_text).collect(),
            })
        }
        "set_cursor_item" => Event::Cursor(sent_stack_text(
            &ClientboundSetCursorItem::decode(r)?.contents,
        )),
        _ => return Ok(None),
    }))
}

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
            action => Event::Command(snake_case(&format!("{action:?}"))),
        },
        "interact" => Event::Interact(ServerboundInteract::decode(r)?.using_secondary_action),
        "use_item" => Event::UseItem,
        "player_action" => match ServerboundPlayerAction::decode(r)?.action {
            PlayerAction::ReleaseUseItem => Event::ReleaseUseItem,
            action => Event::Action(snake_case(&format!("{action:?}"))),
        },
        "container_click" => {
            let packet = ServerboundContainerClick::decode(r)?;
            Event::Click(Click {
                container: packet.container_id.0,
                state: packet.state_seqno.0,
                slot: i16::try_from(packet.click.clicked)?,
                button: packet.click.button,
                input: input_name(packet.click.kind).to_owned(),
                cursor: hashed_stack_text(packet.carried_item.as_ref()),
                changed: packet
                    .changed_slots
                    .iter()
                    .map(|(slot, stack)| ChangedSlot {
                        slot: *slot,
                        stack: hashed_stack_text(stack.as_ref()),
                    })
                    .collect(),
            })
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

fn attribute(snapshot: &AttributeSnapshot<'_>) -> anyhow::Result<Event> {
    let Some(known) = snapshot.attribute.resolve() else {
        anyhow::bail!("unknown attribute id {}", snapshot.attribute);
    };
    let modifiers = snapshot
        .modifiers
        .iter()
        .map(|modifier| Modifier {
            id: modifier.id.to_string(),
            amount: modifier.amount,
            operation: u8::from(modifier.operation),
        })
        .collect();
    Ok(Event::Attribute(Attribute {
        name: known.as_static_str().to_owned(),
        base: snapshot.base,
        modifiers,
    }))
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

fn entity_data(entity: DataEntity, r: &mut &[u8]) -> Vec<Event> {
    let mut events = Vec::new();
    loop {
        let entry = (|| -> anyhow::Result<Option<(u8, i32, MetaDataValue<'_>)>> {
            let index = u8::decode(r)?;
            if index == 0xFF {
                return Ok(None);
            }
            let serializer = VarInt::decode(&mut &r[..])?.0;
            Ok(Some((index, serializer, MetaDataValue::decode(r)?)))
        })();
        let (index, serializer, value) = match entry {
            Ok(Some(entry)) => entry,
            Ok(None) => return events,
            Err(error) => {
                println!("set_entity_data of {entity:?}: {error}; the later entries are not kept");
                return events;
            }
        };
        let undecoded = matches!(
            value,
            MetaDataValue::Particle | MetaDataValue::Particles | MetaDataValue::ResolvableProfile
        );
        events.push(Event::EntityData(EntityData {
            entity,
            index,
            value: data_text(serializer, &value),
        }));
        if undecoded {
            return events;
        }
    }
}

fn data_text(serializer: i32, value: &MetaDataValue<'_>) -> String {
    match value {
        MetaDataValue::Byte(byte) => format!("byte:{:#04x}", byte.cast_unsigned()),
        MetaDataValue::VarInt(int) => format!("int:{}", int.0),
        MetaDataValue::Float(float) => format!("float:{float}"),
        MetaDataValue::Boolean(boolean) => format!("bool:{boolean}"),
        MetaDataValue::OptionalBlockPos(None) => "block_pos:-".to_owned(),
        MetaDataValue::BlockPos(at) | MetaDataValue::OptionalBlockPos(Some(at)) => {
            format!("block_pos:{},{},{}", at.x, at.y, at.z)
        }
        MetaDataValue::Pose(pose) => format!("pose:{}", snake_case(&format!("{pose:?}"))),
        _ => format!("serializer:{serializer}"),
    }
}

fn input_name(kind: ClickKind) -> &'static str {
    match kind {
        ClickKind::PickUp => "pickup",
        ClickKind::QuickMove => "quick_move",
        ClickKind::Swap => "swap",
        ClickKind::Clone => "clone",
        ClickKind::Throw => "throw",
        ClickKind::Drag => "quick_craft",
        ClickKind::PickUpAll => "pickup_all",
    }
}

fn snake_case(name: &str) -> String {
    let mut text = String::new();
    for (position, letter) in name.char_indices() {
        if letter.is_ascii_uppercase() && position > 0 {
            text.push('_');
        }
        text.push(letter.to_ascii_lowercase());
    }
    text
}
