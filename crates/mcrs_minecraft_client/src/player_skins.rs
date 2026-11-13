//! The skins of the players that the server adds: the model and the skin address come from the
//! profile in the player list, and the native client downloads the image.

use std::collections::HashMap;

use bevy::prelude::*;
use mcrs_minecraft_client_entity::ClientEntity;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_render::PlayerSkin;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_profile::{PlayerModelType, SkinTextures};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundPlayerInfoRemove, ClientboundPlayerInfoUpdate,
};

/// Gives each player entity the skin of its profile.
pub struct PlayerSkinsPlugin;

impl Plugin for PlayerSkinsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProfileSkins>()
            .add_observer(on_player_info_update)
            .add_observer(on_player_info_remove)
            .add_systems(Update, dress_players);
        #[cfg(not(target_family = "wasm"))]
        app.add_systems(Update, download::finish_downloads);
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
/// arrives.
fn dress_players(
    mut commands: Commands,
    mut skins: ResMut<ProfileSkins>,
    added: Query<(Entity, &ClientEntity), Added<ClientEntity>>,
    all: Query<(Entity, &ClientEntity)>,
    mut worn: Query<&mut PlayerSkin>,
) {
    let updated = std::mem::take(&mut skins.updated);
    // chisle: a profile update scans the client entities; an index by UUID lifts it.
    let again = (!updated.is_empty())
        .then(|| {
            all.iter()
                .filter(|(_, client)| updated.contains(&client.uuid.as_u128()))
        })
        .into_iter()
        .flatten();
    for (entity, client) in added.iter().chain(again) {
        if client.kind != EntityType::Player {
            continue;
        }
        let textures = skins.by_player.get(&client.uuid.as_u128());
        let model = textures.map_or(PlayerModelType::Wide, |textures| textures.model);
        #[cfg(not(target_family = "wasm"))]
        let fetch = textures.and_then(|textures| download::trusted(&textures.url));
        #[cfg(target_family = "wasm")]
        // chisle: the web client downloads no skin; a fetch through the browser lifts it.
        let fetch: Option<String> = None;
        match worn.get_mut(entity) {
            Ok(mut skin) => {
                skin.model = model;
                if fetch.is_none() {
                    skin.image = None;
                }
            }
            Err(_) => {
                commands
                    .entity(entity)
                    .insert(PlayerSkin { model, image: None });
            }
        }
        #[cfg(not(target_family = "wasm"))]
        {
            let mut player = commands.entity(entity);
            player.remove::<download::SkinDownload>();
            if let Some(url) = fetch {
                player.insert(download::SkinDownload::start(url));
            }
        }
    }
}

#[cfg(not(target_family = "wasm"))]
mod download {
    use std::io::Read as _;
    use std::time::Duration;

    use bevy::prelude::*;
    use bevy::tasks::{IoTaskPool, Task, futures::check_ready};
    use mcrs_minecraft_entity_render::{PlayerSkin, SkinImage};

    /// The largest skin file that the client reads.
    const MAX_BYTES: u64 = 1 << 20;

    /// The download of the skin image of a player entity.
    #[derive(Component)]
    pub(super) struct SkinDownload(Task<Option<SkinImage>>);

    impl SkinDownload {
        // chisle: one blocking request on a thread of the IO pool for each skin; an async HTTP
        // client lifts it.
        pub(super) fn start(url: String) -> Self {
            Self(IoTaskPool::get().spawn(async move {
                let agent: ureq::Agent = ureq::Agent::config_builder()
                    .timeout_global(Some(Duration::from_secs(15)))
                    .build()
                    .into();
                let mut response = agent.get(&url).call().ok()?;
                let mut bytes = Vec::new();
                response
                    .body_mut()
                    .as_reader()
                    .take(MAX_BYTES)
                    .read_to_end(&mut bytes)
                    .ok()?;
                SkinImage::from_png(&bytes)
                    .inspect_err(|error| warn!("skin {url}: {error}"))
                    .ok()
            }))
        }
    }

    /// The HTTPS address that the client downloads a skin of `url` from, or `None` for an
    /// address off the texture server of the game, the only server of skins.
    pub(super) fn trusted(url: &str) -> Option<String> {
        let rest = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))?;
        rest.starts_with("textures.minecraft.net/")
            .then(|| format!("https://{rest}"))
    }

    pub(super) fn finish_downloads(
        mut commands: Commands,
        mut downloads: Query<(Entity, &mut SkinDownload, &mut PlayerSkin)>,
    ) {
        for (entity, mut download, mut skin) in &mut downloads {
            let Some(image) = check_ready(&mut download.0) else {
                continue;
            };
            if image.is_some() {
                skin.image = image;
            }
            commands.entity(entity).remove::<SkinDownload>();
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn only_the_texture_server_of_the_game_serves_skins_and_always_over_https() {
            let cases = [
                (
                    "http://textures.minecraft.net/texture/a",
                    Some("https://textures.minecraft.net/texture/a"),
                ),
                (
                    "https://textures.minecraft.net/texture/a",
                    Some("https://textures.minecraft.net/texture/a"),
                ),
                ("https://textures.minecraft.net.evil.example/a", None),
                ("https://evil.example/textures.minecraft.net/a", None),
                ("file:///textures.minecraft.net/a", None),
            ];
            for (url, expected) in cases {
                assert_eq!(super::trusted(url).as_deref(), expected, "{url}");
            }
        }
    }
}
