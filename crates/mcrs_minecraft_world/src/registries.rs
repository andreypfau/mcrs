use mcrs_minecraft_registry::static_report::from_report;
use mcrs_minecraft_registry::{LoadReport, RegistrySet};

pub fn static_registries(report: &[u8]) -> Result<RegistrySet, LoadReport> {
    from_report(report).map_err(|error| {
        let mut report = LoadReport::new();
        report.invalid_report(error);
        report
    })
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
}
