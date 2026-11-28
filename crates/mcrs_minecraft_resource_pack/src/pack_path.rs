use mcrs_minecraft_resource::ResourceLocation;

#[must_use]
pub fn pack_path(location: ResourceLocation<'_>, folder: &str, extension: &str) -> String {
    format!(
        "{}/{folder}/{}.{extension}",
        location.namespace(),
        location.path()
    )
}
