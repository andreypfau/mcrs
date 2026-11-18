use alloc::vec::Vec;

use mcrs_minecraft_item_stack::Profile;
use mcrs_minecraft_profile::ProfileIdentity;
use mcrs_minecraft_text::Color;
use mcrs_minecraft_text::IntoText;
use mcrs_minecraft_text::Text;

pub(crate) fn profile_part(profile: &Profile) -> Option<Text> {
    let ProfileIdentity::Partial {
        name,
        id,
        properties,
    } = &profile.profile
    else {
        return None;
    };
    let dynamic = properties.is_empty() && name.is_some() != id.is_some();
    dynamic.then(|| Text::translate("component.profile.dynamic", Vec::new()).color(Color::GRAY))
}
