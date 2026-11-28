use bevy_ecs::message::MessageReader;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_enchantment::enchantment_effect_component_type_keys::EnchantmentEffectComponentType;
use mcrs_minecraft_engine::PlayerBody;
use mcrs_minecraft_engine_damage::Died;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_inventory_player::ArmourSlot;
use mcrs_minecraft_inventory_player::MAIN;
use mcrs_minecraft_inventory_player::OFF_HAND;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_entity::HandVelocity;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_player_holders::EnchantmentEffects;
use mcrs_minecraft_vanilla_item_entity::Form;
use mcrs_minecraft_vanilla_item_entity::ItemEntityCommandsExt;
use mcrs_minecraft_vanilla_item_entity::Launch;

const EQUIPMENT_ORDER: [u16; 5] = [
    OFF_HAND,
    ArmourSlot::Feet.index(),
    ArmourSlot::Legs.index(),
    ArmourSlot::Chest.index(),
    ArmourSlot::Head.index(),
];

pub(crate) fn drop_inventory(
    mut deaths: MessageReader<Died>,
    rules: Option<Res<GameRules>>,
    effects: Res<EnchantmentEffects>,
    players: Query<
        (
            &Slots,
            PlayerBody,
            Option<&EntityUuid>,
            Option<&PlayerGameMode>,
        ),
        With<Player>,
    >,
    enchantments: Query<&Enchantments>,
    mut commands: Commands,
) {
    let keep = rules.is_some_and(|rules| rules.is_on(GameRule::KeepInventory));
    if keep {
        deaths.clear();
        return;
    }
    for death in deaths.read() {
        let Ok((slots, body, uuid, game_mode)) = players.get(death.target) else {
            continue;
        };
        if game_mode.is_some_and(|mode| mode.0 == GameMode::Spectator) {
            continue;
        }
        let launch = Launch {
            source: BirthSource::Entity(uuid.copied().unwrap_or_default()),
            form: Form::Hand {
                feet: death.position,
                eye_height: body.dimensions().eye_height,
                velocity: HandVelocity::Scatter,
            },
            values: ItemEntity::with_pickup_delay(Cause::PlayerDeath.pickup_delay()),
        };
        let order = MAIN.chain(EQUIPMENT_ORDER);
        for stack in order.filter_map(|index| slots.get(usize::from(index))) {
            let vanishes = enchantments.get(stack).is_ok_and(|enchantments| {
                effects.any_has(
                    enchantments,
                    EnchantmentEffectComponentType::PreventEquipmentDrop,
                )
            });
            if vanishes {
                commands.entity(stack).despawn();
            } else {
                commands.entity(stack).launch(launch);
            }
        }
    }
}
