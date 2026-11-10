/// The path below `assets/` of the resource `<namespace>:<path>` in `folder`, with `extension`.
#[must_use]
pub fn resource_path(id: &str, folder: &str, extension: &str) -> String {
    let (namespace, path) = split_id(id);
    format!("{namespace}/{folder}/{path}.{extension}")
}

/// The namespace and the path of an identifier. An identifier with no namespace is in
/// `minecraft`.
#[must_use]
pub fn split_id(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or(("minecraft", id))
}

/// The identifier in its full form, `<namespace>:<path>`.
#[must_use]
pub fn full_id(id: &str) -> String {
    let (namespace, path) = split_id(id);
    format!("{namespace}:{path}")
}
