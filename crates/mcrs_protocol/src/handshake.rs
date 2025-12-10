use mcrs_protocol_macros::{Decode, Encode};

#[derive(Clone, Copy, Debug, Encode, Decode)]
pub enum Intent {
    #[packet(tag = 1)]
    Status = 1,
    #[packet(tag = 2)]
    Login = 2,
    #[packet(tag = 3)]
    Transfer = 3,
}
