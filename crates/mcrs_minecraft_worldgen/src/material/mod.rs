pub mod compile;
pub mod eval;
#[cfg(test)]
mod oracle;
pub mod proto;

pub use compile::{MaterialInputs, SurfaceNoise};
pub use eval::{MaterialEval, MaterialScratch, NO_WATER};
pub use proto::{MaterialConditionHolder, MaterialRuleHolder};
