use mcrs_minecraft_damage::DamageTags;
use mcrs_minecraft_damage::DamageTypeRules;
use mcrs_minecraft_entity::damage_type::DamageType;
use mcrs_minecraft_entity::damage_type_keys;
use mcrs_minecraft_entity::registry_keys::DAMAGE_TYPE;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::ResourceKey;

use crate::scene;
use crate::scene::Executor;

#[test]
fn the_bake_sets_the_bits_of_the_damage_type_tags() {
    let mut app = scene::app(Executor::Single);
    let registries = test_registries();
    app.insert_resource(CurrentRegistries::new(registries.clone()));
    scene::tick(&mut app);
    let rules = app.world().resource::<DamageTypeRules>();
    let tags = |key: ResourceKey<'static, DamageType>| {
        let id = registries
            .id_of(DAMAGE_TYPE.location(), key.location())
            .expect("a vanilla damage type");
        rules.tags(u32::from(id))
    };
    let fall = tags(damage_type_keys::FALL);
    assert!(fall.contains(DamageTags::IS_FALL), "fall is in is_fall");
    assert!(
        !fall.contains(DamageTags::IS_FIRE),
        "fall is not in is_fire"
    );
    assert!(
        tags(damage_type_keys::IN_FIRE).contains(DamageTags::IS_FIRE),
        "in_fire is in is_fire"
    );
    assert_eq!(
        tags(damage_type_keys::CACTUS),
        DamageTags::NO_KNOCKBACK,
        "cactus has only no_knockback of the baked tags"
    );
}
