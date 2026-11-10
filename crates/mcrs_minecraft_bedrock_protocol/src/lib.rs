//! Bedrock Edition protocol implementation: 0xFE batch framing, compression, login, and pack negotiation.

// std: flate2

mod batch;
mod compression_algorithm;
mod disconnect;
mod error;
mod login;
mod network_settings;
mod packet_header;
pub mod packet_ids;
mod play_status;
mod request_network_settings;
mod resource_pack_client_response;
mod resource_pack_stack;
mod resource_packs_info;
mod version;

pub use batch::{BATCH_PACKET_ID, BatchCompression, decode_batch, encode_batch};
pub use compression_algorithm::CompressionAlgorithm;
pub use disconnect::Disconnect;
pub use error::ProtocolError;
pub use login::Login;
pub use network_settings::NetworkSettings;
pub use packet_header::{read_packet_header, write_packet_header};
pub use play_status::{PlayStatus, PlayStatusType};
pub use request_network_settings::RequestNetworkSettings;
pub use resource_pack_client_response::{ResourcePackClientResponse, ResourcePackResponseStatus};
pub use resource_pack_stack::{Experiment, ResourcePackStack, ResourcePackStackEntry};
pub use resource_packs_info::{ResourcePackInfoEntry, ResourcePacksInfo};
pub use version::{MINECRAFT_VERSION, PROTOCOL_VERSION};
