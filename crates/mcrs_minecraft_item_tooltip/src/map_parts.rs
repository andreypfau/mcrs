use alloc::vec;
use alloc::vec::Vec;

use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::MapId;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_text::Color;
use mcrs_minecraft_text::IntoText;
use mcrs_minecraft_text::ItemHover;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_text::TranslateArg;

use crate::TooltipDetail;
use crate::TooltipInputs;
use crate::visible::Visible;

pub(crate) fn map_parts<N: NameLookup>(
    visible: &Visible<'_, N>,
    map_id: MapId,
    inputs: &TooltipInputs<'_>,
    parts: &mut Vec<Text>,
) {
    let Some(map) = inputs.maps.iter().find(|map| map.id == map_id) else {
        parts.push(Text::translate("filled_map.unknown", Vec::new()).color(Color::GRAY));
        return;
    };
    if visible
        .items
        .effective::<CustomName>(visible.stack)
        .is_none()
    {
        parts.push(gray("filled_map.id", vec![TranslateArg::from(map_id.0)]));
    }
    if map.locked {
        parts.push(gray("filled_map.locked", Vec::new()));
    }
    if inputs.detail == TooltipDetail::Advanced {
        let scale = i32::from(map.scale);
        if let Some(size) = 2_i32.checked_pow(u32::from(map.scale)) {
            parts.push(gray("filled_map.scale", vec![TranslateArg::from(size)]));
            parts.push(gray(
                "filled_map.level",
                vec![TranslateArg::from(scale), TranslateArg::from(4)],
            ));
        }
    }
}

fn gray(key: &'static str, with: Vec<TranslateArg<ItemHover>>) -> Text {
    Text::translate_args(key, with).color(Color::GRAY)
}
