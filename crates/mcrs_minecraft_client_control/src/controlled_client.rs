use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryData;
use bevy_ecs::system::SystemParam;
use bevy_math::DVec3;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_chat::MAX_INPUT;
use mcrs_minecraft_chat_client::Outgoing;
use mcrs_minecraft_chat_client::Signing;
use mcrs_minecraft_client_world::attack_input::AttackInput;
use mcrs_minecraft_client_world::block_breaking;
use mcrs_minecraft_client_world::block_breaking::Targets;
use mcrs_minecraft_client_world::block_target;
use mcrs_minecraft_client_world::block_target::Reach;
use mcrs_minecraft_client_world::chat::SendChat;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_client_world::food_level::FoodLevel;
use mcrs_minecraft_client_world::game_mode::LocalGameMode;
use mcrs_minecraft_client_world::joined_game::JoinedGame;
use mcrs_minecraft_client_world::local_motion;
use mcrs_minecraft_client_world::local_motion::LocalMotion;
use mcrs_minecraft_client_world::movement_input::MovementInput;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::player_load::PlayerLoad;
use mcrs_minecraft_client_world::scripted_input::ScriptedInput;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine_world::entity::physics::OldTransform;
use mcrs_minecraft_engine_world::entity::physics::Rotation;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_visual::ClientEntity;
use mcrs_minecraft_entity_visual::Motion;
use mcrs_minecraft_entity_visual::dimensions;
use mcrs_minecraft_item::SelectedHotbarSlot;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::BlockPos;
use mcrs_voxel::ColumnPos;

use crate::meshed::Meshed;
use crate::reply;

pub(crate) const TERRAIN_RADIUS: i32 = 2;

pub(crate) const MAX_ENTITIES: usize = 32;

#[derive(QueryData)]
#[query_data(mutable)]
pub(crate) struct OwnPlayer {
    transform: &'static mut Transform,
    old: &'static mut OldTransform,
    motion: Option<&'static LocalMotion>,
    attributes: Option<&'static AttributeInstances>,
    vitals: Option<&'static Vitals>,
    food: Option<&'static FoodLevel>,
    mode: Option<&'static LocalGameMode>,
    slot: Option<&'static mut SelectedHotbarSlot>,
    load: Option<&'static PlayerLoad>,
}

#[derive(SystemParam)]
pub(crate) struct ControlledClient<'w, 's> {
    connections: Query<'w, 's, (&'static ConnectionState, Has<JoinedGame>)>,
    player: Query<'w, 's, OwnPlayer, With<Player>>,
    entities: Query<'w, 's, Targets, Without<Player>>,
    columns: Option<Res<'w, ColumnStore>>,
    blocks: Option<Res<'w, Blocks>>,
    meshed: Option<Res<'w, Meshed>>,
    scripted: Option<ResMut<'w, ScriptedInput>>,
    chat: Option<ResMut<'w, Messages<SendChat>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Missing {
    Player,
    Entity,
    Block,
    Slot,
    Chat,
    EmptyMessage,
    LongMessage,
}

impl Missing {
    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::Player => "the own player is not in the world yet",
            Self::Entity => "no entity with that network id",
            Self::Block => "the block is not loaded",
            Self::Slot => "the own player has no hotbar yet",
            Self::Chat => "this client has no chat",
            Self::EmptyMessage => "the message is empty",
            Self::LongMessage => "the message is longer than 256 characters",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Button {
    Up,
    Click,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Chat {
    Chat,
    Command,
}

impl ControlledClient<'_, '_> {
    pub(crate) fn ready(&self) -> reply::Ready {
        let joined = self
            .connections
            .iter()
            .any(|(state, joined)| joined && *state == ConnectionState::Game);
        let player = self.player.single().ok().filter(|_| joined);
        let placed = player
            .as_ref()
            .is_some_and(|player| player.load == Some(&PlayerLoad::Loaded));
        let terrain = placed
            && player.is_some_and(|player| {
                let center = ColumnPos::from(player.transform.translation);
                self.columns.as_deref().is_some_and(|columns| {
                    center
                        .square(TERRAIN_RADIUS)
                        .all(|column| columns.holds(column))
                })
            });
        let meshing = match self.meshed.as_deref() {
            None => reply::Meshing::NotRendered,
            Some(Meshed(false)) => reply::Meshing::Pending,
            Some(Meshed(true)) => reply::Meshing::Done,
        };
        reply::Ready {
            joined,
            placed,
            terrain,
            meshing,
            world_ready: terrain && meshing != reply::Meshing::Pending,
        }
    }

    pub(crate) fn player(&self) -> Result<reply::PlayerState, Missing> {
        let player = self.player.single().map_err(|_| Missing::Player)?;
        let translation = player.transform.translation;
        Ok(reply::PlayerState {
            x: translation.x,
            y: translation.y,
            z: translation.z,
            yaw: player.transform.rotation.yaw(),
            pitch: player.transform.rotation.pitch(),
            health: player.vitals.map(|vitals| vitals.health),
            food: player.food.map(|food| food.food),
            game_mode: player.mode.map(|mode| mode.current.name().to_owned()),
            on_ground: player
                .motion
                .is_some_and(|motion| motion.0.body.flags.on_ground),
            slot: player.slot.map(|slot| slot.0),
        })
    }

    pub(crate) fn target(&self) -> Result<reply::Target, Missing> {
        let player = self.player.single().map_err(|_| Missing::Player)?;
        let position = player.transform.translation;
        let pick = match (
            player.attributes.and_then(Reach::from_instances),
            self.blocks.as_deref(),
            self.columns.as_deref(),
        ) {
            (Some(reach), Some(blocks), Some(columns)) => block_breaking::pick(
                player.transform,
                player.motion,
                reach,
                blocks,
                columns,
                &self.entities,
            ),
            _ => None,
        };
        Ok(match pick {
            None => reply::Target::None,
            Some(block_target::Target::Entity(hit)) => self.entities.get(hit.entity).map_or(
                reply::Target::None,
                |(_, client, motion, _)| {
                    reply::Target::Entity(entity_state(client, motion, position))
                },
            ),
            Some(block_target::Target::Block(hit)) => reply::Target::Block {
                block: self.block(hit.pos)?,
                face: hit.face.name().to_owned(),
            },
        })
    }

    pub(crate) fn entities(&self, radius: f64) -> Result<Vec<reply::EntityState>, Missing> {
        let position = self.position()?;
        let mut entities: Vec<reply::EntityState> = self
            .entities
            .iter()
            .map(|(_, client, motion, _)| entity_state(client, motion, position))
            .filter(|entity| entity.distance <= radius)
            .collect();
        entities.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        entities.truncate(MAX_ENTITIES);
        Ok(entities)
    }

    pub(crate) fn block(&self, pos: BlockPos) -> Result<reply::BlockState, Missing> {
        let (Some(columns), Some(blocks)) = (self.columns.as_deref(), self.blocks.as_deref())
        else {
            return Err(Missing::Block);
        };
        let state = columns.block(pos).map_err(|_| Missing::Block)?;
        let block = blocks.owner(state).ok_or(Missing::Block)?;
        Ok(reply::BlockState {
            x: pos.x,
            y: pos.y,
            z: pos.z,
            block: block.identifier.as_str().to_owned(),
            state: state.number(),
        })
    }

    pub(crate) fn look(&mut self, yaw: f32, pitch: f32) -> Result<(), Missing> {
        let mut player = self.player.single_mut().map_err(|_| Missing::Player)?;
        let rotation = Rotation::new(yaw, pitch.clamp(-90.0, 90.0));
        player.transform.rotation = rotation;
        player.old.0.rotation = rotation;
        Ok(())
    }

    pub(crate) fn look_at(&mut self, point: DVec3) -> Result<(), Missing> {
        let player = self.player.single().map_err(|_| Missing::Player)?;
        let eye = player.transform.translation
            + DVec3::Y * f64::from(local_motion::eye_height(player.motion));
        let to = point - eye;
        let yaw = (-to.x).atan2(to.z).to_degrees() as f32;
        let pitch = (-to.y).atan2(to.x.hypot(to.z)).to_degrees() as f32;
        self.look(yaw, pitch)
    }

    pub(crate) fn look_at_entity(&mut self, network_id: i32) -> Result<(), Missing> {
        let center = self
            .entities
            .iter()
            .find(|(_, client, ..)| client.network_id == network_id)
            .map(|(_, client, motion, data)| {
                let height = dimensions(client, data).map_or(0.0, |size| size.height);
                motion.current.position + DVec3::Y * f64::from(height) / 2.0
            })
            .ok_or(Missing::Entity)?;
        self.look_at(center)
    }

    pub(crate) fn select_slot(&mut self, slot: u8) -> Result<(), Missing> {
        let player = self.player.single_mut().map_err(|_| Missing::Player)?;
        let mut selected = player.slot.ok_or(Missing::Slot)?;
        selected.set_if_neq(SelectedHotbarSlot(slot));
        Ok(())
    }

    pub(crate) fn hold(&mut self, keys: MovementInput) {
        if let Some(scripted) = self.scripted.as_mut() {
            let movement = ScriptedInput {
                movement: keys,
                ..**scripted
            };
            scripted.set_if_neq(movement);
        }
    }

    pub(crate) fn press(&mut self, button: Button) {
        let attack = AttackInput {
            presses: u32::from(button == Button::Click),
            down: button != Button::Up,
            screen_open: false,
        };
        if let Some(scripted) = self.scripted.as_mut() {
            let pressed = ScriptedInput {
                attack,
                ..**scripted
            };
            scripted.set_if_neq(pressed);
        }
    }

    pub(crate) fn release(&mut self) {
        if let Some(scripted) = self.scripted.as_mut() {
            scripted.set_if_neq(ScriptedInput::default());
        }
    }

    pub(crate) fn send(&mut self, text: &str, kind: Chat) -> Result<(), Missing> {
        let chat = self.chat.as_mut().ok_or(Missing::Chat)?;
        let mut line = String::new();
        mcrs_minecraft_chat::normalize(text, &mut line);
        if line.is_empty() {
            return Err(Missing::EmptyMessage);
        }
        chat.write(SendChat(match kind {
            Chat::Chat
                if text.chars().count() > MAX_INPUT || line.encode_utf16().count() > MAX_INPUT =>
            {
                return Err(Missing::LongMessage);
            }
            Chat::Chat => Outgoing::Chat(line),
            Chat::Command => Outgoing::Command {
                line,
                signing: Signing::Unsigned,
            },
        }));
        Ok(())
    }

    fn position(&self) -> Result<DVec3, Missing> {
        self.player
            .single()
            .map(|player| player.transform.translation)
            .map_err(|_| Missing::Player)
    }
}

fn entity_state(client: &ClientEntity, motion: &Motion, from: DVec3) -> reply::EntityState {
    let position = motion.current.position;
    reply::EntityState {
        id: client.network_id,
        kind: client.kind.as_static_str().to_owned(),
        x: position.x,
        y: position.y,
        z: position.z,
        distance: position.distance(from),
    }
}
