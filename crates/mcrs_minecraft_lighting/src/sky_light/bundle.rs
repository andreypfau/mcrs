//! Sky-light bundle. The implementation currently lives in
//! `crate::bundle`; this module re-exports the sky-side surface so
//! callers can land on `crate::sky_light::bundle::SkyLightBundle`
//! as the canonical path. A future refactor will move the body here.

pub use crate::bundle::SkyLightBundle;
