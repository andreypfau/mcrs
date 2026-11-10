use std::sync::LazyLock;

use mcrs_minecraft_client_jar::Progress;
use mcrs_minecraft_model::resource_set::{ResourceSet, is_resource};

/// The resource set of the installed client jar, read once per test binary.
pub fn client_pack() -> &'static ResourceSet {
    static SET: LazyLock<ResourceSet> = LazyLock::new(|| {
        mcrs_minecraft_client_jar::resolve(&Progress::default(), is_resource, drop)
            .unwrap()
            .0
            .into_iter()
            .collect()
    });
    &SET
}
