//! The download of the skin image of a player entity from the texture server of the game.

use std::io::Read as _;
use std::time::Duration;

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, futures::check_ready};
use mcrs_minecraft_entity_render::{PlayerSkin, SkinImage};

/// The largest skin file that the client reads.
const MAX_BYTES: u64 = 1 << 20;

/// The download of the skin image of a player entity.
#[derive(Component)]
pub(crate) struct SkinDownload(Task<Option<SkinImage>>);

impl SkinDownload {
    // chisle: one blocking request on a thread of the IO pool for each skin; an async HTTP
    // client lifts it.
    pub(crate) fn start(url: String) -> Self {
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
pub(crate) fn trusted(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    rest.starts_with("textures.minecraft.net/")
        .then(|| format!("https://{rest}"))
}

pub(crate) fn finish_downloads(
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
