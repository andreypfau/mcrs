#[must_use]
pub fn resource_path(id: &str, folder: &str, extension: &str) -> String {
    let (namespace, path) = split_id(id);
    format!("{namespace}/{folder}/{path}.{extension}")
}

#[must_use]
pub fn split_id(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or(("minecraft", id))
}

#[must_use]
pub fn full_id(id: &str) -> String {
    let (namespace, path) = split_id(id);
    format!("{namespace}:{path}")
}
