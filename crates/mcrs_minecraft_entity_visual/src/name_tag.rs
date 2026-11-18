use bevy_ecs::component::Component;
use mcrs_minecraft_registries::Named;

pub type NameText = mcrs_minecraft_item::Text<Named>;

/// The name that a host gives to an entity (CEREN-NAME-005). It acts as the custom name and, for
/// a player, replaces the profile name. The host shows the name of a mob with
/// `SyncedData::name_visible`.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct NameTag(pub NameText);

/// The name text of an entity: the name of the host, else the custom name of the server, else the
/// profile name (CEREN-NAME-003, item 1). A mob passes no profile name.
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
