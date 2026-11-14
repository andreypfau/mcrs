//! The skins of the players that the server adds: the model and the skin address come from the
//! profile in the player list, and the native client downloads the image.

use std::collections::HashMap;

use bevy::prelude::*;
use mcrs_minecraft_client_entity::{ClientEntity, HostEntity};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_render::{PlayerModel, PlayerSkin};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_profile::{PlayerModelType, SkinTextures};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundPlayerInfoRemove, ClientboundPlayerInfoUpdate,
};

#[cfg(not(target_family = "wasm"))]
use crate::skin_download::{SkinDownload, finish_downloads, trusted};

/// Gives each player entity the skin of its profile.
pub struct PlayerSkinsPlugin;

impl Plugin for PlayerSkinsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProfileSkins>()
            .add_observer(on_player_info_update)
            .add_observer(on_player_info_remove)
            .add_systems(Update, dress_players);
        // The commands of `dress_players` apply first, so that an old download that ends in the
        // same frame does not remove the new one.
        #[cfg(not(target_family = "wasm"))]
        app.add_systems(Update, finish_downloads.after(dress_players));
    }
}

/// The skin that the profile of each listed player names, by the UUID of the player, and the
/// players whose profile came again since the last frame.
#[derive(Resource, Default)]
struct ProfileSkins {
    by_player: HashMap<u128, SkinTextures>,
    updated: Vec<u128>,
}

fn on_player_info_update(
    event: On<Received<ClientboundPlayerInfoUpdate<'static>>>,
    mut skins: ResMut<ProfileSkins>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    if !packet.actions.add_player() {
        return;
    }
    for entry in packet.entries.iter() {
        let uuid = entry.player_uuid.as_u128();
        match SkinTextures::of(&entry.properties) {
            Some(textures) => skins.by_player.insert(uuid, textures),
            None => skins.by_player.remove(&uuid),
        };
        skins.updated.push(uuid);
    }
}

/// A player that leaves the list keeps the skin of its entity: a server can hide a player that
/// stays in the world from the list.
fn on_player_info_remove(
    event: On<Received<ClientboundPlayerInfoRemove>>,
    mut skins: ResMut<ProfileSkins>,
) {
    if let Some(packet) = event.packet() {
        for uuid in &packet.players {
            skins.by_player.remove(&uuid.as_u128());
        }
    }
}

/// Gives each new player entity, and each player whose profile came again, the model of its
/// profile and starts the download of its image. A player keeps its image until the new one
/// arrives. A player of the host keeps the skin that the host gives.
fn dress_players(
    mut commands: Commands,
    mut skins: ResMut<ProfileSkins>,
    added: Query<(Entity, &ClientEntity), (Added<ClientEntity>, Without<HostEntity>)>,
    all: Query<(Entity, &ClientEntity), Without<HostEntity>>,
    mut models: Query<&mut PlayerModel>,
) {
    let updated = std::mem::take(&mut skins.updated);
    // chisle: a profile update scans the client entities; an index by UUID lifts it.
    let again = (!updated.is_empty())
        .then(|| {
            all.iter().filter(|(entity, client)| {
                !added.contains(*entity) && updated.contains(&client.uuid.as_u128())
            })
        })
        .into_iter()
        .flatten();
    for (entity, client) in added.iter().chain(again) {
        if client.kind != EntityType::Player {
            continue;
        }
        let textures = skins.by_player.get(&client.uuid.as_u128());
        let model = PlayerModel(textures.map_or(PlayerModelType::Wide, |textures| textures.model));
        #[cfg(not(target_family = "wasm"))]
        let fetch = textures.and_then(|textures| trusted(&textures.url));
        #[cfg(target_family = "wasm")]
        // chisle: the web client downloads no skin; a fetch through the browser lifts it.
        let fetch: Option<String> = None;
        let mut player = commands.entity(entity);
        match models.get_mut(entity) {
            Ok(mut worn) => {
                worn.set_if_neq(model);
            }
            Err(_) => {
                player.insert(model);
            }
        }
        if fetch.is_none() {
            player.remove::<PlayerSkin>();
        }
        #[cfg(not(target_family = "wasm"))]
        {
            player.remove::<SkinDownload>();
            if let Some(url) = fetch {
                player.insert(SkinDownload::start(url));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce as _;
    use mcrs_minecraft_client_entity::{Placement, host_living_entity};
    use mcrs_minecraft_entity_render::SkinImage;

    use super::*;

    /// CPR-HOST-001, item 4: the profile of the server never dresses a player of the host.
    #[test]
    fn a_player_of_the_host_keeps_its_skin_and_a_player_of_the_server_takes_its_profile() {
        let mut world = World::new();
        world.init_resource::<ProfileSkins>();
        let skin = PlayerSkin(SkinImage::from_rgba(64, 64, vec![255; 64 * 64 * 4]).unwrap());
        let model = PlayerModel(PlayerModelType::Slim);
        let mut spawn = || {
            let player =
                host_living_entity(EntityType::Player, Default::default(), Placement::default());
            world.spawn((player.unwrap(), model, skin.clone())).id()
        };
        let host = spawn();
        let server = spawn();
        world.entity_mut(server).remove::<HostEntity>();
        world.run_system_once(dress_players).unwrap();
        assert_eq!(world.get::<PlayerModel>(host), Some(&model));
        assert_eq!(world.get::<PlayerSkin>(host), Some(&skin));
        assert_eq!(
            world.get::<PlayerModel>(server),
            Some(&PlayerModel(PlayerModelType::Wide))
        );
        assert_eq!(world.get::<PlayerSkin>(server), None);
    }
}
