use bevy_app::App;
use bevy_asset::{AssetServer, Assets};
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_vanilla_worldgen::{TemplateAsset, pool_templates, template_handle};
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_feature::proto::StructureProcessorList;
use mcrs_minecraft_worldgen_structure::{Structure, StructureSet};

pub fn the_structure_registries_land_before_playing(app: &App) {
    let world = app.world();

    assert_eq!(world.resource::<Assets<TemplateAsset>>().len(), 1511);

    let set = world.resource::<CurrentRegistries>();
    assert_eq!(
        set.registry::<StructureSet>()
            .expect("the structure sets are parsed by the loader")
            .len(),
        21
    );
    assert_eq!(
        set.registry::<Structure>()
            .expect("the structures are parsed by the loader")
            .len(),
        52
    );
    assert_eq!(
        set.registry::<StructureProcessorList>()
            .expect("the processor lists are parsed by the loader")
            .len(),
        40
    );
    let pools = set
        .registry::<TemplatePool>()
        .expect("the template pools are parsed by the loader");
    assert_eq!(pools.len(), 245);

    let only_named_beside_a_missing_sibling = world
        .resource::<AssetServer>()
        .get_handle::<TemplateAsset>(
            "minecraft/structure/ancient_city/walls/intact_horizontal_wall_bridge.nbt",
        )
        .expect("every shipped template was requested");
    assert!(
        world
            .resource::<Assets<TemplateAsset>>()
            .get(&only_named_beside_a_missing_sibling)
            .is_some()
    );

    let no_corners = pools
        .by_name(
            &"minecraft:ancient_city/walls/no_corners"
                .parse::<ResourceLocationBuf>()
                .unwrap(),
        )
        .and_then(|id| pools.get(id))
        .expect("the pool is a loaded entry");
    let missing =
        ResourceLocationBuf::minecraft("ancient_city/walls/intact_horizontal_wall_stairs_5")
            .unwrap();
    assert!(
        pool_templates(no_corners).contains(&missing),
        "a pool naming a template that does not ship still loads"
    );
    assert!(
        template_handle(world.resource::<AssetServer>(), &missing).is_none(),
        "a template that does not ship is never requested"
    );
}
