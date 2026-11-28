use bevy_ecs::component::Component;
use mcrs_minecraft_registries::Named;

pub type NameText = mcrs_minecraft_item::Text<Named>;

#[derive(Component, Clone, Debug, PartialEq)]
pub struct NameTag(pub NameText);

#[must_use]
pub fn name_text<'a>(
    host: Option<&'a NameTag>,
    custom: Option<&'a NameText>,
    profile: Option<&'a NameText>,
) -> Option<&'a NameText> {
    host.map(|tag| &tag.0).or(custom).or(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_beats_the_custom_name_and_the_custom_name_beats_the_profile() {
        let [host, custom, profile] = ["host", "custom", "profile"].map(NameText::text);
        let tag = NameTag(host.clone());
        let pick = |host, custom, profile| name_text(host, custom, profile);
        assert_eq!(pick(Some(&tag), Some(&custom), Some(&profile)), Some(&host));
        assert_eq!(pick(None, Some(&custom), Some(&profile)), Some(&custom));
        assert_eq!(pick(None, None, Some(&profile)), Some(&profile));
        assert_eq!(pick(None, None, None), None);
    }
}
