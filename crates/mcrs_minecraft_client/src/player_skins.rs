use std::collections::HashMap;

use bevy::prelude::*;
use mcrs_minecraft_client_gui::inventory_figure::InventoryFigure;
use mcrs_minecraft_client_world::registries::SessionSet;
use mcrs_minecraft_client_world::session::session_boundary;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_render::{PlayerModel, PlayerSkin};
use mcrs_minecraft_entity_visual::{ClientEntity, HostEntity, NameText};
use mcrs_minecraft_network::client::ConnectionEnded;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_profile::{PlayerModelType, SkinTextures};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundPlayerInfoRemove, ClientboundPlayerInfoUpdate,
};

#[cfg(not(target_family = "wasm"))]
use crate::skin_download::{SkinDownload, finish_downloads, trusted};

pub struct PlayerSkinsPlugin;

impl Plugin for PlayerSkinsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ConnectionEnded>()
            .init_resource::<ProfileSkins>()
            .add_observer(on_player_info_update)
            .add_observer(on_player_info_remove)
            .add_systems(
                Update,
                (
                    clear_profile_skins
                        .in_set(SessionSet::Clear)
                        .run_if(session_boundary),
                    dress_players.after(SessionSet::Clear).run_if(dress_due),
                ),
            );
        #[cfg(not(target_family = "wasm"))]
        app.add_systems(Update, finish_downloads.after(dress_players));
    }
}

#[derive(Resource, Default)]
pub struct ProfileSkins {
    by_player: HashMap<u128, SkinTextures>,
    names: HashMap<u128, NameText>,
    updated: Vec<u128>,
}

impl ProfileSkins {
    #[must_use]
    pub fn name(&self, uuid: u128) -> Option<&NameText> {
        self.names.get(&uuid)
    }
}

fn clear_profile_skins(mut skins: ResMut<ProfileSkins>) {
    skins.by_player.clear();
    skins.names.clear();
    skins.updated.clear();
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
        skins
            .names
            .insert(uuid, NameText::text(entry.username.to_owned()));
        match SkinTextures::of(&entry.properties) {
            Some(textures) => skins.by_player.insert(uuid, textures),
            None => skins.by_player.remove(&uuid),
        };
        skins.updated.push(uuid);
    }
}

fn on_player_info_remove(
    event: On<Received<ClientboundPlayerInfoRemove>>,
    mut skins: ResMut<ProfileSkins>,
) {
    if let Some(packet) = event.packet() {
        for uuid in &packet.players {
            skins.by_player.remove(&uuid.as_u128());
            skins.names.remove(&uuid.as_u128());
        }
    }
}

type Dressed = Or<(Without<HostEntity>, With<InventoryFigure>)>;

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
fn dress_due(skins: Res<ProfileSkins>, added: Query<(), (Added<ClientEntity>, Dressed)>) -> bool {
    !skins.updated.is_empty() || !added.is_empty()
}

fn dress_players(
    mut commands: Commands,
    mut skins: ResMut<ProfileSkins>,
    added: Query<(Entity, &ClientEntity), (Added<ClientEntity>, Dressed)>,
    all: Query<(Entity, &ClientEntity), Dressed>,
    mut models: Query<&mut PlayerModel>,
) {
    let updated = std::mem::take(&mut skins.updated);
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
        let model = match textures.map_or(PlayerModelType::Wide, |textures| textures.model) {
            PlayerModelType::Wide => PlayerModel::Wide,
            PlayerModelType::Slim => PlayerModel::Slim,
        };
        #[cfg(not(target_family = "wasm"))]
        let fetch = textures.and_then(|textures| trusted(&textures.url));
        #[cfg(target_family = "wasm")]
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
    use mcrs_minecraft_entity_render::SkinImage;
    use mcrs_minecraft_entity_visual::{Placement, host_living_entity};

    use super::*;

    #[test]
    fn host_player_keeps_skin_and_server_player_takes_profile() {
        let mut world = World::new();
        world.init_resource::<ProfileSkins>();
        let skin = PlayerSkin(SkinImage::from_rgba(64, 64, vec![255; 64 * 64 * 4]).unwrap());
        let model = PlayerModel::Slim;
        let mut spawn = || {
            let player =
                host_living_entity(EntityType::Player, Default::default(), Placement::default());
            world.spawn((player.unwrap(), model, skin.clone())).id()
        };
        let host = spawn();
        let server = spawn();
        let figure = spawn();
        world.entity_mut(server).remove::<HostEntity>();
        world.entity_mut(figure).insert(InventoryFigure);
        world.run_system_once(dress_players).unwrap();
        assert_eq!(world.get::<PlayerModel>(host), Some(&model));
        assert_eq!(world.get::<PlayerSkin>(host), Some(&skin));
        assert_eq!(world.get::<PlayerModel>(server), Some(&PlayerModel::Wide));
        assert_eq!(world.get::<PlayerSkin>(server), None);
        assert_eq!(
            world.get::<PlayerModel>(figure),
            Some(&PlayerModel::Wide),
            "the inventory figure takes the profile of the local player"
        );
    }
}
