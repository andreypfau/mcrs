//! Bedrock Edition protocol: batch framing, batch compression and the handshake packets.

// std: flate2

mod batch_compression;
mod batch_decoder;
mod batch_encoder;
mod compression_algorithm;
mod connection_request;
mod disconnect;
mod login;
mod network_settings;
mod packet_list;
mod play_status;
mod protocol_error;
mod request_network_settings;
mod resource_pack_client_response;
mod resource_pack_stack;
mod resource_packs_info;
mod version;

pub use batch_compression::BatchCompression;
pub use batch_decoder::{BatchDecoder, DEFAULT_INFLATE_LIMIT};
pub use batch_encoder::BatchEncoder;
pub use compression_algorithm::CompressionAlgorithm;
pub use connection_request::ConnectionRequest;
pub use disconnect::{Disconnect, DisconnectReason, DisconnectScreen};
pub use login::Login;
pub use network_settings::NetworkSettings;
pub use packet_list::PacketList;
pub use play_status::{PlayStatus, PlayStatusKind};
pub use protocol_error::ProtocolError;
pub use request_network_settings::RequestNetworkSettings;
pub use resource_pack_client_response::{ResourcePackClientResponse, ResourcePackResponse};
pub use resource_pack_stack::{Experiment, ResourcePackStack, ResourcePackStackEntry};
pub use resource_packs_info::{PackCdnUrl, ResourcePackInfoEntry, ResourcePacksInfo};
pub use version::{MINECRAFT_VERSION, PROTOCOL_VERSION};
