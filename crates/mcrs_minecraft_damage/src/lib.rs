#![no_std]

extern crate alloc;

mod damage_source;
mod damage_tags;
mod damage_type_rules;
mod defence;
mod footing;
mod hit;
mod hit_outcome;
mod hit_record;
mod hit_report;
mod hurt;
mod hurt_rules;
mod knockback;
mod vitals;

pub use damage_source::DamageSource;
pub use damage_tags::DamageTags;
pub use damage_type_rules::DamageTypeRules;
pub use defence::Defence;
pub use footing::Footing;
pub use hit::Hit;
pub use hit_outcome::HitOutcome;
pub use hit_record::HitRecord;
pub use hit_record::LastHit;
pub use hit_report::HitReport;
pub use hurt::hurt;
pub use hurt::preview;
pub use hurt_rules::HurtRules;
pub use knockback::knockback;
pub use vitals::Vitals;
