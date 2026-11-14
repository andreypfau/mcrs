use bevy::prelude::*;
use mcrs_minecraft_profile::PlayerModelType;
use mcrs_molang::string_hash;

use crate::entity_assets::EntityAssets;
use crate::entity_library::TextureImage;
use crate::frame_queries::VariantEntry;
use crate::skin_image::{SKIN_SIDE, SkinImage};

/// The skins that the entity textures hold at once. An entity past them draws the default skin
/// of its model.
// chisle: a fixed set of 256 layers of 64 by 64; a texture array that grows lifts it.
pub const SKIN_SLOTS: u16 = 256;

/// The arm model of a player entity (`docs/spec/client-player-render.md`). Without a
/// [`PlayerSkin`], the entity draws the default skin of its model.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerModel(pub PlayerModelType);

/// The skin image of a player entity with a [`PlayerModel`].
#[derive(Component, Clone, Debug, PartialEq)]
pub struct PlayerSkin(pub SkinImage);

/// The skin layer of the entity textures that holds the image of a [`PlayerSkin`].
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SkinSlot(pub(crate) u16);

/// The texels of a skin layer that the render world writes.
#[derive(Clone, Debug)]
pub(crate) struct SkinUpload {
    pub(crate) variant: u16,
    pub(crate) image: SkinImage,
}

/// The skin layers in use and the writes that the next frame sends.
#[derive(Resource, Default)]
pub(crate) struct SkinSlots {
    free: Vec<u16>,
    next: u16,
    uploads: Vec<SkinUpload>,
    generation: Option<u32>,
    /// An entity found no free layer, so a layer that frees goes to it.
    waiting: bool,
}

impl SkinSlots {
    pub(crate) fn take_uploads(&mut self) -> Vec<SkinUpload> {
        std::mem::take(&mut self.uploads)
    }

    fn allocate(&mut self) -> Option<u16> {
        self.free.pop().or_else(|| {
            let slot = self.next;
            (slot < SKIN_SLOTS).then(|| {
                self.next += 1;
                slot
            })
        })
    }
}

/// The texture identifier of a skin layer.
pub(crate) fn skin_identifier(slot: u16) -> String {
    format!("mcrs:skin/{slot}")
}

/// The skin layers, in slot order. A layer with no texels starts transparent, so each skin face
/// has the cutout and draws with the alpha test.
pub(crate) fn skin_textures() -> impl Iterator<Item = TextureImage> {
    (0..SKIN_SLOTS).map(|slot| TextureImage {
        identifier: skin_identifier(slot),
        width: SKIN_SIDE,
        height: SKIN_SIDE,
        rgba: Vec::new(),
    })
}

/// The variant entry that the render controllers of a player read: the model type, and the
/// texture of its skin layer as the asset, or `""` while the entity has none.
pub(crate) fn skin_entry(
    model: PlayerModel,
    slot: Option<&SkinSlot>,
    assets: &EntityAssets,
) -> VariantEntry {
    let empty = string_hash("");
    let asset = match slot {
        Some(slot) if !assets.library.skin_variants.is_empty() => {
            string_hash(&skin_identifier(slot.0))
        }
        _ => empty,
    };
    VariantEntry {
        id: 0.0,
        identifier: empty,
        asset,
        model: string_hash(match model.0 {
            PlayerModelType::Wide => "wide",
            PlayerModelType::Slim => "slim",
        }),
    }
}

/// Gives each entity with a [`PlayerSkin`] a skin layer and sends its texels to the render world.
/// New entity assets start with empty layers, so each skin goes again. An entity that found no
/// free layer tries again when one frees. An entity that loses its [`PlayerSkin`] frees its layer.
pub(crate) fn assign_skins(
    mut commands: Commands,
    assets: Option<Res<EntityAssets>>,
    mut slots: ResMut<SkinSlots>,
    mut removed: RemovedComponents<PlayerSkin>,
    changed: Query<(Entity, &PlayerSkin, Option<&SkinSlot>), Changed<PlayerSkin>>,
    all: Query<(Entity, Ref<PlayerSkin>, Option<&SkinSlot>)>,
    worn: Query<(), (With<SkinSlot>, Without<PlayerSkin>)>,
) {
    for entity in removed.read() {
        if worn.contains(entity) {
            commands.entity(entity).remove::<SkinSlot>();
        }
    }
    let Some(assets) = assets.filter(|assets| !assets.library.skin_variants.is_empty()) else {
        return;
    };
    let rebuilt = slots.generation != Some(assets.generation);
    slots.generation = Some(assets.generation);
    let first = assets.library.skin_variants.start;
    let retry = slots.waiting && !slots.free.is_empty();
    if retry {
        slots.waiting = false;
    }
    let mut assign = |(entity, skin, slot): (Entity, &PlayerSkin, Option<&SkinSlot>)| {
        let Some(number) = slot.map(|slot| slot.0).or_else(|| slots.allocate()) else {
            warn_once!("more than {SKIN_SLOTS} player skins; the rest draw a default skin");
            slots.waiting = true;
            return;
        };
        if slot.is_none() {
            commands.entity(entity).insert(SkinSlot(number));
        }
        slots.uploads.push(SkinUpload {
            variant: first + number,
            image: skin.0.clone(),
        });
    };
    if rebuilt || retry {
        for (entity, skin, slot) in &all {
            if rebuilt || skin.is_changed() || slot.is_none() {
                assign((entity, &skin, slot));
            }
        }
    } else {
        for entity in &changed {
            assign(entity);
        }
    }
}

pub(crate) fn release_skin(
    removed: On<Remove<SkinSlot>>,
    skins: Query<&SkinSlot>,
    mut slots: ResMut<SkinSlots>,
) {
    if let Ok(slot) = skins.get(removed.entity) {
        slots.free.push(slot.0);
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_entity::entity_type_keys::EntityType;
    use mcrs_minecraft_entity_visual::{EntityLight, Placement, host_living_entity};

    use super::*;
    use crate::{EntityPack, EntityPasses, EntityRenderPlugin, EntitySlot, SlotArena};

    fn texture(identifier: &str) -> TextureImage {
        TextureImage {
            identifier: identifier.into(),
            width: SKIN_SIDE,
            height: SKIN_SIDE,
            rgba: vec![255; (SKIN_SIDE * SKIN_SIDE * 4) as usize],
        }
    }

    #[test]
    fn a_player_draws_its_skin_layer_and_the_default_skin_of_its_model_without_a_skin() {
        let mut assets = EntityAssets::default();
        let defaults = ["wide/steve", "slim/alex"]
            .map(|name| texture(&format!("minecraft:entity/player/{name}")));
        assets.add_pack(EntityPack::repository(), defaults.to_vec(), 2048);
        assert!(
            assets.pack.diagnostics.is_empty(),
            "{:?}",
            assets.pack.diagnostics
        );
        let (definition, player) = assets.pack.entity("minecraft:player").unwrap();
        let tables = &assets.library.definitions[definition];
        assert!(tables.program.is_ok(), "{:?}", tables.program);
        let model = |name: &str| {
            let at = player
                .resources
                .geometries
                .iter()
                .position(|(geometry, _)| geometry.as_str() == name)
                .unwrap();
            tables.models[at].unwrap()
        };
        let (wide, slim) = (model("default"), model("slim"));
        let alex = assets.library.textures["minecraft:entity/player/slim/alex"];
        let first_skin = assets.library.skin_variants.start;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, EntityRenderPlugin))
            .insert_resource(assets);
        app.finish();
        app.insert_resource(EntityPasses::ON);
        let player =
            host_living_entity(EntityType::Player, Default::default(), Placement::default());
        let skin = || PlayerSkin(SkinImage::from_rgba(64, 64, vec![0; 64 * 64 * 4]).unwrap());
        let entity = app
            .world_mut()
            .spawn((player.unwrap(), PlayerModel(PlayerModelType::Slim), skin()))
            .id();
        let layer = |app: &App| {
            let slot = app.world().get::<EntitySlot>(entity).unwrap().slot;
            app.world()
                .resource::<SlotArena>()
                .state(slot)
                .unwrap()
                .layers[0]
        };
        let uploads = |app: &App| app.world().resource::<SkinSlots>().uploads.len();
        app.update();
        assert_eq!(layer(&app), [slim, first_skin]);
        assert_eq!(uploads(&app), 1);

        app.world_mut().get_mut::<PlayerModel>(entity).unwrap().0 = PlayerModelType::Wide;
        app.update();
        assert_eq!(layer(&app), [wide, first_skin]);
        assert_eq!(uploads(&app), 1, "a new model sends the same texels again");

        // CPR-HOST-001, item 5: a player that loses its skin frees its layer.
        app.world_mut().get_mut::<PlayerModel>(entity).unwrap().0 = PlayerModelType::Slim;
        app.world_mut().entity_mut(entity).remove::<PlayerSkin>();
        app.update();
        assert_eq!(layer(&app), [slim, alex]);
        assert_eq!(app.world().get::<SkinSlot>(entity), None);
        assert_eq!(app.world().resource::<SkinSlots>().free, [0]);

        app.world_mut().entity_mut(entity).insert(skin());
        app.update();
        assert_eq!(layer(&app), [slim, first_skin]);
        assert!(app.world().resource::<SkinSlots>().free.is_empty());

        let light = |app: &App| {
            let slot = app.world().get::<EntitySlot>(entity).unwrap().slot;
            let state = app.world().resource::<SlotArena>().state(slot).unwrap();
            (state.block_light, state.sky_light)
        };
        assert_eq!(light(&app), (0, 15), "an entity with no light has open sky");
        app.world_mut()
            .entity_mut(entity)
            .insert(EntityLight { block: 7, sky: 3 });
        app.update();
        assert_eq!(light(&app), (7, 3));
    }
}
