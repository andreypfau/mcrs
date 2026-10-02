pub mod corpus;
pub mod gradle;
pub mod registries;
pub mod release;

#[cfg(test)]
mod tests {
    #[test]
    fn the_descriptor_names_the_corpus_version() {
        assert_eq!(
            mcrs_minecraft_client_jar::RELEASE.id,
            mcrs_minecraft_core::VERSION.id
        );
    }
}
