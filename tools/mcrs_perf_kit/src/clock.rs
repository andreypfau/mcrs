use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

#[must_use]
pub fn utc_now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}
