//! Backward-compat re-export of `LightTicket`. The canonical home is
//! `crate::world::lifecycle::ticket`.
pub use crate::world::lifecycle::ticket::LightTicket;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_ticket_marker_compile_test() {
        let _m = LightTicket;
    }
}
