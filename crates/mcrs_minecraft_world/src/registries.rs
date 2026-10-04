use crate::entity::minecraft::EntityIds;
use mcrs_minecraft_item::SoundEvent;
use mcrs_minecraft_registry::key::Block;
use mcrs_minecraft_registry::static_report::from_report;
use mcrs_minecraft_registry::{LoadReport, RegistrySet};

pub fn static_registries(report: &[u8]) -> Result<(RegistrySet, EntityIds), LoadReport> {
    let set = from_report(report).map_err(|error| {
        let mut report = LoadReport::new();
        report.invalid_report(error);
        report
    })?;
    let mut missing = LoadReport::new();
    missing.registry::<SoundEvent>(&set);
    missing.registry::<Block>(&set);
    let entity_ids = EntityIds::resolve(&set, &mut missing);
    match entity_ids {
        Some(entity_ids) if missing.is_empty() => Ok((set, entity_ids)),
        _ => Err(missing),
    }
}

pub fn refuse(report: &LoadReport) -> ! {
    tracing::error!("the registries report is unusable:\n{report}");
    std::process::exit(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_malformed_report_is_refused_with_a_report() {
        let json = br#"{"minecraft:x":{"protocol_id":0,"entries":{"a:a":{"protocol_id":0},"a:b":{"protocol_id":0}}}}"#;
        let report = static_registries(json)
            .err()
            .expect("a shared id is refused");
        assert!(!report.is_empty());
        let text = report.to_string();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.contains("minecraft:x"), "{text}");
    }

    fn refused_without(registry: &str) -> LoadReport {
        let mut report: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../assets/mcrs/reports/registries.json"
        ))
        .unwrap();
        report
            .as_object_mut()
            .unwrap()
            .remove(registry)
            .unwrap_or_else(|| panic!("the report carries {registry}"));
        let bytes = serde_json::to_vec(&report).unwrap();

        static_registries(&bytes)
            .err()
            .unwrap_or_else(|| panic!("a report without {registry} is refused"))
    }

    #[test]
    fn a_report_without_sound_events_is_refused() {
        let refused = refused_without("minecraft:sound_event");
        assert!(
            refused.to_string().contains("minecraft:sound_event"),
            "{refused}"
        );
    }

    #[test]
    fn a_report_without_blocks_is_refused() {
        let refused = refused_without("minecraft:block");
        assert!(refused.to_string().contains("minecraft:block"), "{refused}");
    }
}
