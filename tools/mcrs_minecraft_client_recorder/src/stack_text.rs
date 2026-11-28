use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::ComponentType;
use mcrs_minecraft_item_stack::HashedStack;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::item::data_component_type_keys::DataComponentType;
use mcrs_minecraft_registries::Wire;

pub(crate) fn hashed_stack_text(stack: Option<&HashedStack>) -> String {
    let Some(stack) = stack else {
        return "-".to_owned();
    };
    let item = Item::from_protocol_id(stack.kind.0);
    let added =
        (stack.values.iter()).map(|value| format!("{}={}", type_name(value.component), value.hash));
    let removed = (stack.removals.iter()).map(|component| format!("!{}", type_name(*component)));
    let components: Vec<String> = added.chain(removed).collect();
    with_components(item, stack.count, &components)
}

pub(crate) fn sent_stack_text(stack: &ProtoStack<Wire>) -> String {
    let item = Item::from_protocol_id(stack.id.number());
    if stack.count <= 0 || item == Some(Item::Air) {
        return "-".to_owned();
    }
    let added = stack
        .components
        .added
        .iter()
        .map(|value| value.kind().as_static_str().to_owned());
    let removed = stack.components.removed.iter().map(removed_text);
    let components: Vec<String> = added.chain(removed).collect();
    with_components(item, stack.count, &components)
}

fn type_name(component: ComponentType) -> &'static str {
    DataComponentType::from_protocol_id(component.0)
        .map_or("#unknown", DataComponentType::as_static_str)
}

fn removed_text(kind: &DataComponentType) -> String {
    format!("!{}", kind.as_static_str())
}

fn with_components(item: Option<Item>, count: i32, components: &[String]) -> String {
    let name = item.map_or("#unknown", Item::as_static_str);
    if components.is_empty() {
        format!("{name}*{count}")
    } else {
        format!("{name}*{count}{{{}}}", components.join(","))
    }
}
