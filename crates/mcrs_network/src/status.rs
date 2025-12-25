pub struct ServerStatusPacketListener<'a> {
    has_requested_status: bool,
    // connection: crate::connect::Connection,
    status: &'a str,
}

const REQUEST_HANDLED_KEY: &str = "multiplayer.status.request_handled";

// impl<'a> ServerStatusPacketListener<'a> {
//     pub(crate) fn new(connection: crate::connect::RawConnection, status: &'a str) -> Self {
//         Self {
//             has_requested_status: false,
//             connection,
//             status,
//         }
//     }
//
//     pub(crate) async fn handle(&mut self) -> anyhow::Result<()> {
//         loop {
//             if self.connection.disconnect_reason.is_some() {
//                 return Ok(());
//             }
//             let packet = self
//                 .connection
//                 .recv_packet::<ServerboundStatusPacket>()
//                 .await?;
//             match packet {
//                 Packet::StatusRequest => self.handle_status_request().await?,
//                 Packet::PingRequest(p) => self.handle_ping_request(p).await?,
//             }
//         }
//     }
//
//     async fn handle_status_request(&mut self) -> anyhow::Result<()> {
//         if self.has_requested_status {
//             self.connection.disconnect_reason = Some(Text::translate(REQUEST_HANDLED_KEY, vec![]));
//             Ok(())
//         } else {
//             let response =
//                 ClientboundStatusPacket::StatusResponse(StatusResponse { json: self.status });
//             self.has_requested_status = true;
//             self.connection.send_packet(&response).await
//         }
//     }
//
//     async fn handle_ping_request(&mut self, packet: PingRequest) -> anyhow::Result<()> {
//         let response = ClientboundStatusPacket::PongResponse(PongResponse {
//             payload: packet.payload,
//         });
//         self.connection.send_packet(&response).await?;
//         self.connection.disconnect_reason = Some(Text::translate(REQUEST_HANDLED_KEY, vec![]));
//         Ok(())
//     }
// }
