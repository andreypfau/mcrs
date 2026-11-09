use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::common_conditions::resource_exists;
use bevy_ecs::schedule::common_conditions::resource_exists_and_changed;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_brain::PanicCauses;
use mcrs_minecraft_engine_brain::MobAi;
use mcrs_minecraft_engine_brain::MobBrainPlugin;
use mcrs_minecraft_engine_brain::MobBrainSet;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_goals::PigBrain;
use mcrs_minecraft_goals::VanillaBrain;
use mcrs_minecraft_item::damage_type::DamageType;
use mcrs_minecraft_item::damage_type_tag_keys::PANIC_CAUSES;
use mcrs_minecraft_registries::CurrentRegistries;

/// The AI of the mobs of one dimension world with the vanilla brains, and the panic causes from
/// the damage type tag `panic_causes` (GOAL-PNC-002).
#[derive(Clone, Copy, Debug)]
pub(crate) struct VanillaBrainsPlugin;

impl Plugin for VanillaBrainsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MobBrainPlugin::new(vanilla_brain));
        app.add_systems(
            FixedUpdate,
            set_panic_causes
                .run_if(
                    resource_exists::<MobAi<VanillaBrain>>
                        .and_then(resource_exists_and_changed::<CurrentRegistries>),
                )
                .after(TickSet::ClientInput)
                .before(MobBrainSet::Prepare),
        );
    }
}

/// The brain of each mob type with goals (GOAL-ANI-001).
fn vanilla_brain(kind: &'static EntityTypeData) -> Option<VanillaBrain> {
    match kind.entity_type {
        EntityType::Pig => Some(VanillaBrain::Pig(PigBrain::default())),
        _ => None,
    }
}

fn set_panic_causes(registries: Res<CurrentRegistries>, mut ai: ResMut<MobAi<VanillaBrain>>) {
    let panic_causes = registries
        .tags::<DamageType>()
        .and_then(|tags| {
            let tag = tags.id(PANIC_CAUSES)?;
            Some(PanicCauses::new(
                tags.members(tag)
                    .filter_map(|id| u32::try_from(id.index()).ok()),
            ))
        })
        .unwrap_or_default();
    ai.set_panic_causes(panic_causes);
}
