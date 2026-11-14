use bevy_app::{App, FixedFirst, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Component, Res, ResMut, Resource, With};
use bevy_ecs::world::World;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_engine_world::entity::physics::{Rotation, Transform};
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_minecraft_save::{
    PlayerDat, player_dat_path, read_player_dat, uuid_to_int_array, write_player_dat,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::{Path, PathBuf};
use tracing::error;

use crate::ability::{PlayerFlying, PlayerGameMode, PlayerOpLevel};
use crate::background_writer::{BackgroundWriter, WriteJob};
use crate::bus::OutboundPlayerReleased;
use crate::client_info::ClientInfo;
use crate::cold_columns::SaveRequest;
use crate::coordinator::clock::GameTime;
use crate::game_mode::ListedGameMode;
use crate::game_profile::GameProfile;
use crate::op_level::AnnouncedOpLevel;
use crate::persisted_registry::{
    CaptureCause, PersistScope, PersistedAppExt, PersistedError, PersistedForm, PersistedRegistry,
    PersistedSnapshot,
};
use crate::player_state::{Ints, PlayerFileState, player_state_keys};
use crate::teleport::TeleportState;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::world_save::WorldSave;

const POSE_KEYS: &[&str] = &["Pos", "Rotation"];
const GAME_TYPE_KEYS: &[&str] = &["playerGameType"];
const ABILITIES_KEY: &str = "abilities";
const ABILITIES_KEYS: &[&str] = &[ABILITIES_KEY];
const DIMENSION_KEYS: &[&str] = &["Dimension"];
const OP_LEVEL_KEYS: &[&str] = &["mcrs:op_level"];
const LISTED_GAME_MODE_KEYS: &[&str] = &["mcrs:listed_game_mode"];
const ANNOUNCED_OP_LEVEL_KEYS: &[&str] = &["mcrs:announced_op_level"];
const PROFILE_KEYS: &[&str] = &["mcrs:profile"];
const CLIENT_INFO_KEYS: &[&str] = &["mcrs:client_info"];
const VIEW_DISTANCE_KEYS: &[&str] = &["mcrs:view_distance"];
const TELEPORT_KEYS: &[&str] = &["mcrs:teleport"];
const UUID_KEYS: &[&str] = &["UUID"];

/// Registers the components of a player that the engine owns, under the root keys of the player
/// file. A plugin that adds a component to the player registers it next to these.
pub struct PlayerPersistencePlugin;

impl Plugin for PlayerPersistencePlugin {
    fn build(&self, app: &mut App) {
        app.register_persisted_codec(POSE_KEYS, PersistScope::Persisted, capture_pose, apply_pose)
            .register_persisted_codec(UUID_KEYS, PersistScope::Persisted, capture_uuid, apply_uuid)
            .register_persisted::<PlayerFileState>(player_state_keys(), PersistScope::Persisted)
            .register_persisted_as::<PlayerGameMode, GameType>(
                GAME_TYPE_KEYS,
                PersistScope::Persisted,
            )
            .register_persisted_codec(
                ABILITIES_KEYS,
                PersistScope::Persisted,
                capture_abilities,
                apply_abilities,
            )
            .register_persisted_codec(
                DIMENSION_KEYS,
                PersistScope::Persisted,
                capture_dimension,
                |_, _, _| Ok(()),
            )
            .register_persisted::<PlayerOpLevel>(OP_LEVEL_KEYS, PersistScope::TravelOnly)
            .register_persisted::<ListedGameMode>(LISTED_GAME_MODE_KEYS, PersistScope::TravelOnly)
            .register_persisted::<AnnouncedOpLevel>(
                ANNOUNCED_OP_LEVEL_KEYS,
                PersistScope::TravelOnly,
            )
            .register_persisted::<GameProfile>(PROFILE_KEYS, PersistScope::TravelOnly)
            .register_persisted::<ClientInfo>(CLIENT_INFO_KEYS, PersistScope::TravelOnly)
            .register_persisted_as::<PlayerViewDistance, ViewDistance>(
                VIEW_DISTANCE_KEYS,
                PersistScope::TravelOnly,
            )
            .register_persisted::<TeleportState>(TELEPORT_KEYS, PersistScope::TravelOnly);
    }
}

/// `Pos` and `Rotation` of the player file. Each is absent from a file that the game wrote
/// without it.
#[derive(Serialize, Deserialize)]
struct Pose {
    #[serde(rename = "Pos", default, skip_serializing_if = "Option::is_none")]
    pos: Option<[f64; 3]>,
    #[serde(rename = "Rotation", default, skip_serializing_if = "Option::is_none")]
    rotation: Option<[f32; 2]>,
}

impl PersistedForm<Transform> for Pose {
    fn from_value(transform: &Transform) -> Self {
        Self {
            pos: Some(transform.translation.to_array()),
            rotation: Some([transform.rotation.yaw(), transform.rotation.pitch()]),
        }
    }

    fn into_value(self) -> Result<Transform, String> {
        let [yaw, pitch] = self.rotation.unwrap_or_default();
        Ok(Transform {
            translation: bevy_math::DVec3::from_array(self.pos.unwrap_or_default()),
            rotation: Rotation::new(yaw, pitch),
        })
    }
}

fn capture_pose(
    world: &mut World,
    player: Entity,
    _: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    let Some(transform) = world.get::<Transform>(player) else {
        return Ok(None);
    };
    mcrs_minecraft_nbt::tag_serializer::to_nbt_tag(&Pose::from_value(transform))
        .map(Some)
        .map_err(|source| PersistedError::Encode { key: "Pos", source })
}

fn apply_pose(world: &mut World, player: Entity, tag: &NbtTag) -> Result<(), PersistedError> {
    let invalid = |message: String| PersistedError::Decode {
        key: "Pos",
        source: mcrs_minecraft_nbt::Error::SerdeError(message),
    };
    let pose: Pose = mcrs_minecraft_nbt::tag_deserializer::from_tag(tag.clone())
        .map_err(|source| PersistedError::Decode { key: "Pos", source })?;
    let mut transform = world.get::<Transform>(player).cloned().unwrap_or_default();
    if let Some(pos) = pose.pos {
        if pos.iter().any(|v| !v.is_finite()) {
            return Err(invalid(format!("Pos {pos:?} is not finite")));
        }
        transform.translation = bevy_math::DVec3::from_array(pos);
    }
    if let Some([yaw, pitch]) = pose.rotation {
        if !yaw.is_finite() || !pitch.is_finite() {
            return Err(invalid(format!("Rotation [{yaw}, {pitch}] is not finite")));
        }
        transform.rotation = Rotation::new(yaw, pitch);
    }
    world
        .get_entity_mut(player)
        .map_err(|_| PersistedError::NoEntity(player))?
        .insert(transform);
    Ok(())
}

/// The file holds the UUID of the profile. A load checks the value and keeps the profile.
fn capture_uuid(
    world: &mut World,
    player: Entity,
    _: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    let Some(profile) = world.get::<GameProfile>(player) else {
        return Ok(None);
    };
    mcrs_minecraft_nbt::tag_serializer::to_nbt_tag(&Ints(uuid_to_int_array(profile.id)))
        .map(Some)
        .map_err(|source| PersistedError::Encode {
            key: "UUID",
            source,
        })
}

fn apply_uuid(_: &mut World, _: Entity, tag: &NbtTag) -> Result<(), PersistedError> {
    mcrs_minecraft_nbt::tag_deserializer::from_tag::<Ints<4>>(tag.clone())
        .map(drop)
        .map_err(|source| PersistedError::Decode {
            key: "UUID",
            source,
        })
}

/// Serde adapter for a [`Transform`] field: it has the shape of the player file.
pub(crate) mod transform_serde {
    use super::*;

    pub(crate) fn serialize<S: Serializer>(
        transform: &Transform,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        Pose::from_value(transform).serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Transform, D::Error> {
        Pose::deserialize(deserializer)?
            .into_value()
            .map_err(serde::de::Error::custom)
    }
}

/// `playerGameType` of the player file: the number of the game mode.
#[derive(Serialize, Deserialize)]
struct GameType(i32);

impl PersistedForm<PlayerGameMode> for GameType {
    fn from_value(mode: &PlayerGameMode) -> Self {
        let index = GameMode::ALL.iter().position(|all| *all == mode.0);
        Self(index.map_or(0, |index| index as i32))
    }

    fn into_value(self) -> Result<PlayerGameMode, String> {
        usize::try_from(self.0)
            .ok()
            .and_then(|index| GameMode::ALL.get(index))
            .map(|mode| PlayerGameMode(*mode))
            .ok_or_else(|| format!("no game type {}", self.0))
    }
}

#[derive(Serialize, Deserialize)]
struct ViewDistance {
    distance: u8,
    vert_distance: u8,
}

impl PersistedForm<PlayerViewDistance> for ViewDistance {
    fn from_value(view: &PlayerViewDistance) -> Self {
        Self {
            distance: view.distance,
            vert_distance: view.vert_distance,
        }
    }

    fn into_value(self) -> Result<PlayerViewDistance, String> {
        Ok(PlayerViewDistance {
            distance: self.distance,
            vert_distance: self.vert_distance,
        })
    }
}

/// The `abilities` compound of the player file. The game reads a missing key with the default.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Abilities {
    invulnerable: bool,
    flying: bool,
    mayfly: bool,
    instabuild: bool,
    #[serde(rename = "mayBuild")]
    may_build: bool,
    #[serde(rename = "flySpeed")]
    fly_speed: f32,
    #[serde(rename = "walkSpeed")]
    walk_speed: f32,
}

impl Default for Abilities {
    fn default() -> Self {
        Self {
            invulnerable: false,
            flying: false,
            mayfly: false,
            instabuild: false,
            may_build: true,
            fly_speed: 0.05,
            walk_speed: 0.1,
        }
    }
}

/// The abilities that the file gave a player, and the game mode that the player had then. The
/// server derives the abilities of a game mode. The values that no rule of the server changes
/// stay as read: the speeds, and `mayfly` while the game mode is the one of the read.
#[derive(Component, Clone, Copy)]
struct ReadAbilities {
    abilities: Abilities,
    mode: Option<GameMode>,
}

fn capture_abilities(
    world: &mut World,
    player: Entity,
    _: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    let (Some(mode), Some(flying)) = (
        world.get::<PlayerGameMode>(player),
        world.get::<PlayerFlying>(player),
    ) else {
        return Ok(None);
    };
    let read = world.get::<ReadAbilities>(player).copied();
    let derived = crate::game_mode::abilities(mode.0, *flying);
    let read_abilities = read.map_or_else(Abilities::default, |read| read.abilities);
    let mayfly = match read {
        Some(read) if read.mode == Some(mode.0) => read_abilities.mayfly || derived.may_fly,
        _ => derived.may_fly,
    };
    mcrs_minecraft_nbt::tag_serializer::to_nbt_tag(&Abilities {
        invulnerable: derived.invulnerable,
        flying: derived.flying,
        mayfly,
        instabuild: derived.instant_build,
        may_build: !mode.0.is_block_placing_restricted(),
        fly_speed: read_abilities.fly_speed,
        walk_speed: read_abilities.walk_speed,
    })
    .map(Some)
    .map_err(|source| PersistedError::Encode {
        key: ABILITIES_KEY,
        source,
    })
}

fn apply_abilities(world: &mut World, player: Entity, tag: &NbtTag) -> Result<(), PersistedError> {
    let abilities: Abilities = mcrs_minecraft_nbt::tag_deserializer::from_tag(tag.clone())
        .map_err(|source| PersistedError::Decode {
            key: ABILITIES_KEY,
            source,
        })?;
    let mode = world.get::<PlayerGameMode>(player).map(|mode| mode.0);
    world
        .get_entity_mut(player)
        .map_err(|_| PersistedError::NoEntity(player))?
        .insert((
            PlayerFlying(abilities.flying),
            ReadAbilities { abilities, mode },
        ));
    Ok(())
}

fn capture_dimension(
    world: &mut World,
    player: Entity,
    _: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    Ok(world
        .get::<InDimension>(player)
        .and_then(|dim| world.get::<ResourceKeyBuf<Dimension>>(dim.0))
        .map(|key| NbtTag::String(key.to_string())))
}

/// Vanilla's own autosave cadence, in ticks.
pub const AUTOSAVE_INTERVAL: u32 = 6000;

/// The one autosave clock. The host advances it at the start of every tick, and every saver
/// reads it: the host files, the data layer of each dimension and the players of each
/// dimension, which get the signal through their input.
#[derive(Resource, Default, Debug)]
pub struct AutosaveCadence {
    ticks: u32,
    due: bool,
}

impl AutosaveCadence {
    /// Whether this tick is an autosave tick. The first tick of a run is not.
    #[must_use]
    pub fn is_due(&self) -> bool {
        self.due
    }

    fn advance(&mut self) {
        self.ticks = (self.ticks + 1) % AUTOSAVE_INTERVAL;
        self.due = self.ticks == 0;
    }
}

pub(crate) fn advance_autosave_cadence(mut cadence: ResMut<AutosaveCadence>) {
    cadence.advance();
}

pub(crate) fn autosave_due(cadence: Res<AutosaveCadence>) -> bool {
    cadence.is_due()
}

/// Runs the [`AutosaveCadence`]. A plugin that saves on the cadence adds it when it is missing.
pub struct AutosavePlugin;

impl Plugin for AutosavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AutosaveCadence>();
        app.add_systems(FixedFirst, advance_autosave_cadence);
    }
}

/// Player files waiting for the I/O pool, and the write that pool is running.
#[derive(Resource, Default)]
pub struct PlayerSaveQueue {
    writer: BackgroundWriter<PlayerBatch>,
    released: Vec<PlayerSession>,
    failed: bool,
}

struct PendingPlayerSave {
    world: PathBuf,
    uuid: Uuid,
    dat: PlayerDat,
    release: Option<PlayerSession>,
}

#[derive(Default)]
struct PlayerBatch(Vec<PendingPlayerSave>);

struct SavedPlayer {
    release: Option<PlayerSession>,
    saved: bool,
}

impl WriteJob for PlayerBatch {
    type Output = Vec<SavedPlayer>;

    fn merge(&mut self, newer: Self) {
        self.0.extend(newer.0);
    }

    fn run(self) -> Vec<SavedPlayer> {
        self.0
            .into_iter()
            .map(|write| {
                let saved = match write_player_dat(&write.world, write.uuid, &write.dat) {
                    Ok(()) => true,
                    Err(err) => {
                        error!(uuid = %write.uuid, "player data not saved: {err}");
                        false
                    }
                };
                SavedPlayer {
                    release: write.release,
                    saved,
                }
            })
            .collect()
    }
}

impl PlayerSaveQueue {
    fn queue(
        &mut self,
        world: PathBuf,
        uuid: Uuid,
        dat: PlayerDat,
        release: Option<PlayerSession>,
    ) {
        let Self {
            writer,
            released,
            failed,
        } = self;
        writer.submit(
            PlayerBatch(vec![PendingPlayerSave {
                world,
                uuid,
                dat,
                release,
            }]),
            |done| Self::finish(released, failed, done),
        );
    }

    fn reap(&mut self) {
        let Self {
            writer,
            released,
            failed,
        } = self;
        writer.reap(|done| Self::finish(released, failed, done));
    }

    fn flush(&mut self) {
        let Self {
            writer,
            released,
            failed,
        } = self;
        writer.flush(|done| Self::finish(released, failed, done));
    }

    fn finish(released: &mut Vec<PlayerSession>, failed: &mut bool, done: Vec<SavedPlayer>) {
        for player in done {
            *failed |= !player.saved;
            released.extend(player.release);
        }
    }

    #[cfg(test)]
    pub(crate) fn gated(
        gate: std::sync::mpsc::Receiver<()>,
        done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        let pool = bevy_tasks::IoTaskPool::get_or_init(bevy_tasks::TaskPool::new);
        let task = pool.spawn(async move {
            let _ = gate.recv();
            done.store(true, std::sync::atomic::Ordering::SeqCst);
            Vec::new()
        });
        Self {
            writer: BackgroundWriter::running(task),
            ..Default::default()
        }
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.writer.is_idle() && self.released.is_empty()
    }

    /// Whether a write failed since the last call.
    pub(crate) fn take_failed(&mut self) -> bool {
        std::mem::take(&mut self.failed)
    }
}

/// The file on disk could not be parsed; saving would replace it with the
/// empty player the session joined with.
#[derive(Component, Debug, Clone, Copy)]
pub struct UnreadablePlayerDat;

impl PersistedSnapshot {
    /// A joining snapshot assembled in one call. The live host builds one with
    /// the `insert_*` methods instead; tests use this to name a player.
    #[doc(hidden)]
    pub fn joining_player(
        uuid: Uuid,
        username: String,
        position: bevy_math::DVec3,
        rotation: bevy_math::Vec2,
        client_info: Option<ClientInfo>,
    ) -> Self {
        let mut snapshot = Self::default();
        snapshot.insert_profile(&GameProfile {
            id: uuid,
            username,
            properties: Vec::new(),
        });
        snapshot.insert_transform(Transform {
            translation: position,
            rotation: Rotation::new(rotation.x, rotation.y),
        });
        if let Some(info) = client_info {
            snapshot.insert_view_distance(PlayerViewDistance {
                distance: info.view_distance,
                vert_distance: PlayerViewDistance::default().vert_distance,
            });
            snapshot.insert_client_info(&info);
        }
        snapshot
    }

    /// The keys of `dat`, which `path` names. A key that no component of `registry` writes to a
    /// player file fails the load.
    pub fn from_dat(
        dat: &PlayerDat,
        registry: &PersistedRegistry,
        path: &Path,
    ) -> Result<Self, PersistedError> {
        let mut snapshot = Self::default();
        for (key, tag) in &dat.root.child_tags {
            let claimed =
                registry
                    .file_key(key)
                    .ok_or_else(|| PersistedError::UnclaimedFileKey {
                        path: path.display().to_string(),
                        key: key.clone(),
                    })?;
            snapshot.insert(claimed, tag.clone());
        }
        Ok(snapshot)
    }

    fn put_logged<T: Serialize>(&mut self, keys: &[&'static str], value: &T) {
        if let Err(err) = self.put(keys, value) {
            error!("player snapshot not encoded: {err}");
        }
    }

    fn get_logged<T: serde::de::DeserializeOwned>(&self, keys: &[&'static str]) -> Option<T> {
        self.get_as(keys).unwrap_or_else(|err| {
            error!("player snapshot not decoded: {err}");
            None
        })
    }

    pub fn insert_profile(&mut self, profile: &GameProfile) {
        self.put_logged(PROFILE_KEYS, profile);
    }

    pub fn insert_client_info(&mut self, info: &ClientInfo) {
        self.put_logged(CLIENT_INFO_KEYS, info);
    }

    pub fn insert_view_distance(&mut self, view: PlayerViewDistance) {
        self.put_logged(VIEW_DISTANCE_KEYS, &ViewDistance::from_value(&view));
    }

    pub fn insert_transform(&mut self, transform: Transform) {
        self.put_logged(POSE_KEYS, &Pose::from_value(&transform));
    }

    pub fn profile(&self) -> Option<GameProfile> {
        self.get_logged(PROFILE_KEYS)
    }

    pub fn client_info(&self) -> Option<ClientInfo> {
        self.get_logged(CLIENT_INFO_KEYS)
    }

    pub fn view_distance(&self) -> Option<PlayerViewDistance> {
        self.get_logged::<ViewDistance>(VIEW_DISTANCE_KEYS)
            .and_then(|form| form.into_value().ok())
    }

    pub fn transform(&self) -> Option<Transform> {
        self.get_logged::<Pose>(POSE_KEYS)
            .filter(|pose| pose.pos.is_some() && pose.rotation.is_some())
            .and_then(|form| form.into_value().ok())
    }

    /// The `Pos` of the file, `None` when it has none, and an error when it is malformed.
    pub fn pos(&self) -> Result<Option<bevy_math::DVec3>, PersistedError> {
        Ok(self
            .get_as::<Pose>(POSE_KEYS)?
            .and_then(|pose| pose.pos)
            .map(bevy_math::DVec3::from_array))
    }

    /// The yaw and pitch of the file, `None` when it has none, and an error when they are
    /// malformed.
    pub fn rotation(&self) -> Result<Option<[f32; 2]>, PersistedError> {
        Ok(self
            .get_as::<Pose>(POSE_KEYS)?
            .and_then(|pose| pose.rotation))
    }

    pub(crate) fn game_mode(&self) -> PlayerGameMode {
        self.get_logged::<GameType>(GAME_TYPE_KEYS)
            .and_then(|form| form.into_value().ok())
            .unwrap_or_default()
    }

    pub(crate) fn flying(&self) -> PlayerFlying {
        PlayerFlying(
            self.get_logged::<Abilities>(ABILITIES_KEYS)
                .is_some_and(|abilities| abilities.flying),
        )
    }

    pub(crate) fn op_level(&self) -> PlayerOpLevel {
        self.get_logged(OP_LEVEL_KEYS).unwrap_or_default()
    }

    /// Sends the player a teleport to `destination` at `departed_at` and returns its id. The
    /// player arrives with the new state.
    pub(crate) fn send_teleport(&mut self, destination: Transform, departed_at: GameTime) -> i32 {
        let mut state = self
            .get_logged::<TeleportState>(TELEPORT_KEYS)
            .unwrap_or_default();
        let id = state.sent(destination, departed_at);
        self.put_logged(TELEPORT_KEYS, &state);
        id
    }

    /// Removes the keys that the arrival of a handover sets itself.
    pub(crate) fn forget_placement(&mut self) {
        for key in POSE_KEYS.iter().chain(DIMENSION_KEYS) {
            self.remove(key);
        }
    }
}

pub fn load_player(world: &mut World, player: Entity) {
    let Some(save) = world.get_resource::<WorldSave>() else {
        return;
    };
    let Some(uuid) = world.get::<GameProfile>(player).map(|profile| profile.id) else {
        return;
    };
    let path = player_dat_path(&save.0, uuid);
    let dat = match read_player_dat(&save.0, uuid) {
        Ok(Some(dat)) => dat,
        Ok(None) => return,
        Err(err) => {
            error!(%uuid, "player data unreadable, joining empty and never saved: {err}");
            world.entity_mut(player).insert(UnreadablePlayerDat);
            return;
        }
    };
    let Some(registry) = world.remove_resource::<PersistedRegistry>() else {
        error!("player data loaded without a persisted-component registry");
        return;
    };
    let result = PersistedSnapshot::from_dat(&dat, &registry, &path)
        .and_then(|snapshot| registry.apply(world, player, &snapshot));
    world.insert_resource(registry);
    if let Err(err) = result {
        error!(%uuid, "player data not loaded, never saving it: {err}");
        world.entity_mut(player).insert(UnreadablePlayerDat);
    }
}

/// Applies the snapshot that the host parsed from the player file at the join.
pub fn apply_join_snapshot(world: &mut World, player: Entity, snapshot: &PersistedSnapshot) {
    if snapshot.is_unreadable() {
        world.entity_mut(player).insert(UnreadablePlayerDat);
        return;
    }
    let Some(registry) = world.remove_resource::<PersistedRegistry>() else {
        error!("player data loaded without a persisted-component registry");
        return;
    };
    let result = registry.apply(world, player, snapshot);
    world.insert_resource(registry);
    if let Err(err) = result {
        error!(?player, "player data not loaded, never saving it: {err}");
        world.entity_mut(player).insert(UnreadablePlayerDat);
    }
}

pub fn save_player(
    world: &mut World,
    player: Entity,
    cause: CaptureCause,
) -> Result<PlayerDat, PersistedError> {
    let Some(registry) = world.remove_resource::<PersistedRegistry>() else {
        return Err(PersistedError::NoRegistry);
    };
    let snapshot = registry.capture_file(world, player, cause);
    world.insert_resource(registry);
    let mut root = mcrs_minecraft_nbt::compound::NbtCompound::new();
    for (key, tag) in snapshot? {
        root.put(key, tag);
    }
    for key in POSE_KEYS {
        if root.get(key).is_none() {
            return Err(PersistedError::Incomplete(key));
        }
    }
    Ok(PlayerDat::new(VERSION.world_version, root))
}

/// The file of `player`, or `None` when the player has no file to write. A leave captures the
/// player even without a file, because its capture closes what the player holds open.
fn player_file(
    world: &mut World,
    player: Entity,
    cause: CaptureCause,
) -> Option<(PathBuf, Uuid, PlayerDat)> {
    let target = file_target(world, player);
    if target.is_none() && cause != CaptureCause::Leave {
        return None;
    }
    let dat = save_player(world, player, cause);
    let (save, uuid) = target?;
    match dat {
        Ok(dat) => Some((save, uuid, dat)),
        Err(err) => {
            error!(%uuid, "player data not encoded: {err}");
            None
        }
    }
}

/// The save folder and the uuid of the file of `player`, or `None` when it has no file to write.
fn file_target(world: &World, player: Entity) -> Option<(PathBuf, Uuid)> {
    let save = world.get_resource::<WorldSave>()?.0.clone();
    let uuid = world.get::<GameProfile>(player)?.id;
    if world.get::<UnreadablePlayerDat>(player).is_some() {
        return None;
    }
    Some((save, uuid))
}

/// Snapshots `player` and queues its file for the I/O pool. The session in `release` is
/// reported as [`OutboundPlayerReleased`] once the file is on disk, or at once when no file is
/// written.
pub fn queue_player_save(
    world: &mut World,
    player: Entity,
    release: Option<PlayerSession>,
    cause: CaptureCause,
) {
    let Some((dir, uuid, dat)) = player_file(world, player, cause) else {
        if let Some(session) = release {
            world.write_message(OutboundPlayerReleased { session });
        }
        return;
    };
    let mut queue = world.get_resource_or_init::<PlayerSaveQueue>();
    queue.queue(dir, uuid, dat, release);
    report_released(world);
}

fn report_released(world: &mut World) {
    let Some(mut queue) = world.get_resource_mut::<PlayerSaveQueue>() else {
        return;
    };
    let released = std::mem::take(&mut queue.released);
    for session in released {
        world.write_message(OutboundPlayerReleased { session });
    }
}

/// Writes a player and waits for the file. A tick queues the write with [`queue_player_save`]
/// instead; this is for tests and tools.
pub fn write_player(world: &mut World, player: Entity, cause: CaptureCause) {
    queue_player_save(world, player, None, cause);
    flush_player_saves(world);
}

/// Snapshots every player on the tick and hands the files to the I/O pool.
pub fn autosave_players(world: &mut World) {
    let players: Vec<Entity> = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .collect();
    for player in players {
        if let Some((dir, uuid, dat)) = player_file(world, player, CaptureCause::Autosave) {
            world
                .get_resource_or_init::<PlayerSaveQueue>()
                .queue(dir, uuid, dat, None);
        }
    }
}

pub(crate) fn player_saves_pending(queue: Res<PlayerSaveQueue>) -> bool {
    !queue.is_idle()
}

/// Collects the finished writes, starts the next batch and reports the players whose file is
/// on disk. A save-all request waits for every write, so the shutdown loses none.
pub fn finish_player_saves(
    mut queue: ResMut<PlayerSaveQueue>,
    request: Option<Res<SaveRequest>>,
    mut released: MessageWriter<OutboundPlayerReleased>,
) {
    if request.is_some_and(|request| *request == SaveRequest::All) {
        queue.flush();
    } else {
        queue.reap();
    }
    for session in queue.released.drain(..) {
        released.write(OutboundPlayerReleased { session });
    }
}

/// Waits for every queued write. For tests and tools.
pub fn flush_player_saves(world: &mut World) {
    if let Some(mut queue) = world.get_resource_mut::<PlayerSaveQueue>() {
        queue.flush();
    }
    report_released(world);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bevy_ecs::component::Component;
    use mcrs_minecraft_protocol::setting::ChatMode;
    use serde::{Deserialize, Serialize};

    #[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Marker(u32);

    pub(crate) fn temp_world() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mcrs-player-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub(crate) fn profile() -> GameProfile {
        GameProfile {
            id: Uuid::from_u128(0x1234),
            username: "persisted".to_owned(),
            properties: Vec::new(),
        }
    }

    pub(crate) fn app(dir: &std::path::Path) -> App {
        let mut app = App::new();
        app.add_plugins(PlayerPersistencePlugin);
        app.register_persisted::<Marker>(&["TestMarker"], PersistScope::Persisted);
        app.insert_resource(WorldSave(dir.to_owned()));
        app
    }

    fn client_info(locale: &str) -> ClientInfo {
        ClientInfo {
            locale: locale.to_owned(),
            view_distance: 8,
            chat_mode: ChatMode::Enabled,
        }
    }

    #[test]
    fn a_plugin_component_survives_a_save_and_a_load() {
        let dir = temp_world();
        let at = Transform::default().with_translation(bevy_math::DVec3::new(3.0, 70.0, -4.0));
        let mut saving = app(&dir);
        let player = saving
            .world_mut()
            .spawn((
                profile(),
                at,
                Marker(9),
                PlayerGameMode(GameMode::Creative),
                PlayerFlying(true),
            ))
            .id();
        write_player(saving.world_mut(), player, CaptureCause::Autosave);

        let mut loading = app(&dir);
        let player = loading.world_mut().spawn(profile()).id();
        load_player(loading.world_mut(), player);

        let world = loading.world();
        assert_eq!(world.get::<Marker>(player), Some(&Marker(9)));
        assert_eq!(world.get::<Transform>(player), Some(&at));
        assert_eq!(
            world.get::<PlayerGameMode>(player),
            Some(&PlayerGameMode(GameMode::Creative))
        );
        assert_eq!(world.get::<PlayerFlying>(player), Some(&PlayerFlying(true)));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_player_file_has_the_vanilla_keys_and_no_session_state() {
        let dir = temp_world();
        let mut app = app(&dir);
        let player = app
            .world_mut()
            .spawn((
                profile(),
                Transform::default().with_translation(bevy_math::DVec3::new(1.0, 2.0, 3.0)),
                PlayerGameMode(GameMode::Adventure),
                PlayerFlying(false),
                PlayerOpLevel(2),
                client_info("en_us"),
                PlayerViewDistance::default(),
                TeleportState::default(),
            ))
            .id();
        write_player(app.world_mut(), player, CaptureCause::Autosave);

        let dat = read_player_dat(&dir, profile().id).unwrap().unwrap();
        assert_eq!(dat.pos(), Some([1.0, 2.0, 3.0]));
        assert_eq!(dat.rotation(), Some([0.0, 0.0]));
        assert_eq!(dat.get("playerGameType"), Some(&NbtTag::Int(2)));
        let abilities = dat
            .get("abilities")
            .and_then(NbtTag::extract_compound)
            .expect("abilities");
        for key in ["invulnerable", "flying", "mayfly", "instabuild", "mayBuild"] {
            assert!(abilities.get_byte(key).is_some(), "{key}");
        }
        for key in ["flySpeed", "walkSpeed"] {
            assert!(abilities.get(key).is_some(), "{key}");
        }
        let keys: Vec<&str> = dat
            .root
            .child_tags
            .iter()
            .map(|(key, _)| key.as_str())
            .collect();
        assert!(
            keys.iter().all(|key| !key.starts_with("mcrs:")),
            "session state in the file: {keys:?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn abilities_without_a_server_rule_come_back_as_read_and_a_mode_change_updates_the_rest() {
        let dir = temp_world();
        let mut dat = PlayerDat::default();
        dat.root.child_tags.clear();
        let mut abilities = mcrs_minecraft_nbt::compound::NbtCompound::new();
        abilities.put_byte("mayfly", 1);
        abilities.put_float("flySpeed", 0.2);
        abilities.put_float("walkSpeed", 0.3);
        dat.root.put("abilities", NbtTag::Compound(abilities));
        dat.root.put("playerGameType", NbtTag::Int(0));
        write_player_dat(&dir, profile().id, &dat).unwrap();

        let mut app = app(&dir);
        let player = app
            .world_mut()
            .spawn((
                profile(),
                Transform::default(),
                PlayerGameMode::default(),
                PlayerFlying(false),
            ))
            .id();
        load_player(app.world_mut(), player);
        let written = |app: &mut App| {
            let dat = save_player(app.world_mut(), player, CaptureCause::Autosave).unwrap();
            let abilities = dat.get("abilities").and_then(NbtTag::extract_compound);
            let abilities = abilities.expect("abilities").clone();
            (
                abilities.get_byte("mayfly"),
                abilities.get_byte("instabuild"),
                abilities.get("flySpeed").cloned(),
                abilities.get("walkSpeed").cloned(),
            )
        };

        assert_eq!(
            written(&mut app),
            (
                Some(1),
                Some(0),
                Some(NbtTag::Float(0.2)),
                Some(NbtTag::Float(0.3))
            )
        );

        app.world_mut()
            .entity_mut(player)
            .insert(PlayerGameMode(GameMode::Creative));
        assert_eq!(
            written(&mut app),
            (
                Some(1),
                Some(1),
                Some(NbtTag::Float(0.2)),
                Some(NbtTag::Float(0.3))
            )
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_load_leaves_the_live_session_state_alone() {
        let dir = temp_world();
        let mut saving = app(&dir);
        let player = saving
            .world_mut()
            .spawn((profile(), Transform::default(), client_info("de_de")))
            .id();
        write_player(saving.world_mut(), player, CaptureCause::Autosave);

        let mut loading = app(&dir);
        let mut live = TeleportState::default();
        let login_id = live.sent(Transform::default(), GameTime(5));
        let player = loading
            .world_mut()
            .spawn((profile(), client_info("fr_fr"), live))
            .id();
        load_player(loading.world_mut(), player);

        let world = loading.world();
        assert_eq!(world.get::<ClientInfo>(player), Some(&client_info("fr_fr")));
        assert_eq!(
            world
                .get::<TeleportState>(player)
                .and_then(|state| state.pending().map(|pending| pending.id)),
            Some(login_id)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unknown_key_fails_the_load_and_is_never_written() {
        for key in ["SomeVanillaKey", "mcrs:not_registered"] {
            let dir = temp_world();
            let mut dat = PlayerDat::default();
            dat.root.put_string(key, "value".to_owned());
            write_player_dat(&dir, profile().id, &dat).unwrap();

            let mut app = app(&dir);
            let player = app
                .world_mut()
                .spawn((profile(), Transform::default()))
                .id();
            load_player(app.world_mut(), player);
            assert!(app.world().get::<UnreadablePlayerDat>(player).is_some());

            write_player(app.world_mut(), player, CaptureCause::Autosave);
            let unchanged = read_player_dat(&dir, profile().id).unwrap().unwrap();
            assert_eq!(
                unchanged.get(key).and_then(NbtTag::extract_string),
                Some("value")
            );
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn the_error_of_an_unknown_key_names_the_file_and_the_key() {
        let mut dat = PlayerDat::default();
        dat.root.put_string("SomeVanillaKey", "value".to_owned());
        let path = std::path::Path::new("world/players/data/x.dat");
        let mut app = app(&temp_world());
        let registry = app.world_mut().resource::<PersistedRegistry>();

        let message = PersistedSnapshot::from_dat(&dat, registry, path)
            .unwrap_err()
            .to_string();

        assert!(message.contains("world/players/data/x.dat"), "{message}");
        assert!(message.contains("SomeVanillaKey"), "{message}");
    }

    #[test]
    fn a_file_without_pos_rotation_and_state_keys_loads_with_the_defaults() {
        let dir = temp_world();
        let mut dat = PlayerDat::default();
        dat.root.child_tags.clear();
        write_player_dat(&dir, profile().id, &dat).unwrap();

        let mut app = app(&dir);
        let arrival = Transform::default().with_translation(bevy_math::DVec3::new(5.0, 6.0, 7.0));
        let player = app
            .world_mut()
            .spawn((profile(), arrival, PlayerFileState::default()))
            .id();
        load_player(app.world_mut(), player);

        let world = app.world();
        assert!(world.get::<UnreadablePlayerDat>(player).is_none());
        assert_eq!(world.get::<Transform>(player), Some(&arrival));
        assert_eq!(
            world.get::<PlayerFileState>(player),
            Some(&PlayerFileState::default())
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_autosave_queue_flushes_every_snapshot_before_the_shutdown_barrier() {
        let dir = temp_world();
        let mut app = app(&dir);
        app.add_plugins(bevy_app::TaskPoolPlugin::default());
        app.init_resource::<PlayerSaveQueue>();
        app.world_mut()
            .spawn((Player, profile(), Transform::default(), Marker(4)));

        autosave_players(app.world_mut());
        flush_player_saves(app.world_mut());
        assert!(read_player_dat(&dir, profile().id).unwrap().is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn autosave_ticks_come_every_interval_and_never_on_the_first() {
        let mut cadence = AutosaveCadence::default();
        let mut due = Vec::new();
        for tick in 1..=2 * AUTOSAVE_INTERVAL {
            cadence.advance();
            if cadence.is_due() {
                due.push(tick);
            }
        }
        assert_eq!(due, [AUTOSAVE_INTERVAL, 2 * AUTOSAVE_INTERVAL]);
    }

    fn saved(release: Option<u64>, ok: bool) -> SavedPlayer {
        SavedPlayer {
            release: release.map(PlayerSession),
            saved: ok,
        }
    }

    fn app_with_running_writes(gate: std::sync::mpsc::Receiver<()>) -> App {
        let pool = bevy_tasks::IoTaskPool::get_or_init(bevy_tasks::TaskPool::new);
        let task = pool.spawn(async move {
            let _ = gate.recv();
            vec![saved(Some(7), true), saved(Some(8), false)]
        });
        let mut app = App::new();
        app.add_message::<OutboundPlayerReleased>();
        app.insert_resource(PlayerSaveQueue {
            writer: BackgroundWriter::running(task),
            ..Default::default()
        });
        app.add_systems(bevy_app::Update, finish_player_saves);
        app
    }

    fn released(app: &mut App) -> Vec<u64> {
        let mut messages = app
            .world_mut()
            .resource_mut::<bevy_ecs::message::Messages<OutboundPlayerReleased>>();
        messages
            .drain()
            .map(|released| released.session.0)
            .collect()
    }

    #[test]
    fn a_departing_player_is_released_when_its_write_finishes_and_a_failure_is_reported() {
        let (open, gate) = std::sync::mpsc::channel();
        let mut app = app_with_running_writes(gate);

        app.update();
        assert!(released(&mut app).is_empty(), "the write has not finished");
        assert!(!app.world().resource::<PlayerSaveQueue>().is_idle());

        open.send(()).unwrap();
        for _ in 0..200 {
            app.update();
            if app.world().resource::<PlayerSaveQueue>().is_idle() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let mut queue = app.world_mut().resource_mut::<PlayerSaveQueue>();
        assert!(queue.is_idle());
        assert!(queue.take_failed(), "a failed write is reported once");
        assert!(!queue.take_failed());
        assert_eq!(
            released(&mut app),
            [7, 8],
            "a failed write still releases the player"
        );
    }

    #[test]
    fn a_save_all_waits_for_the_queued_writes() {
        let (open, gate) = std::sync::mpsc::channel();
        let mut app = app_with_running_writes(gate);
        app.insert_resource(SaveRequest::All);
        open.send(()).unwrap();

        app.update();
        assert!(app.world().resource::<PlayerSaveQueue>().is_idle());
        assert_eq!(released(&mut app), [7, 8]);
    }

    #[test]
    fn a_join_applies_the_snapshot_that_the_host_parsed_and_does_not_read_the_file_again() {
        let dir = temp_world();
        let mut dat = PlayerDat::default();
        dat.root.put("TestMarker", NbtTag::Int(7));
        write_player_dat(&dir, profile().id, &dat).unwrap();
        let mut app = app(&dir);
        let path = player_dat_path(&dir, profile().id);
        let registry = app.world().resource::<PersistedRegistry>();
        let snapshot = PersistedSnapshot::from_dat(&dat, registry, &path).unwrap();
        std::fs::remove_file(&path).unwrap();

        let player = app.world_mut().spawn(profile()).id();
        apply_join_snapshot(app.world_mut(), player, &snapshot);

        assert_eq!(app.world().get::<Marker>(player), Some(&Marker(7)));
        assert!(app.world().get::<UnreadablePlayerDat>(player).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unreadable_join_snapshot_marks_the_player() {
        let mut app = app(&temp_world());
        let player = app.world_mut().spawn(profile()).id();
        apply_join_snapshot(app.world_mut(), player, &PersistedSnapshot::unreadable());
        assert!(app.world().get::<UnreadablePlayerDat>(player).is_some());
    }
}
