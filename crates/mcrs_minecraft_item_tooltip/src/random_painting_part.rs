use alloc::vec::Vec;

use mcrs_minecraft_item_stack::BuiltinKey;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::PaintingVariant;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::rl;
use mcrs_minecraft_text::Color;
use mcrs_minecraft_text::IntoText;
use mcrs_minecraft_text::Text;

use crate::visible::Visible;

const PAINTING: ResourceLocation<'static> = rl!("minecraft:painting");

pub(crate) fn random_painting_part<N: NameLookup>(visible: &Visible<'_, N>) -> Option<Text> {
    let painting = visible.items.id(NameTable::Item, PAINTING)?;
    let random = visible.stack.kind.0 == painting
        && visible
            .items
            .effective::<PaintingVariant>(visible.stack)
            .is_none()
        && visible.shown(BuiltinKey::PaintingVariant);
    random.then(|| Text::translate("painting.random", Vec::new()).color(Color::GRAY))
}
