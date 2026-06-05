use crate::SharedNetworkState;
use crate::intent::handle_intent;
use crate::packet_io::PacketIo;
use log::{error, info, warn};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::timeout;

const HANDLE_CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) async fn start_accept_loop(shared: SharedNetworkState) {
    let listener = match TcpListener::bind(shared.0.address).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("Failed to bind to address {} {}", shared.0.address, e);
            return;
        }
    };
    info!("Listening on {}", shared.0.address);

    loop {
        match listener.accept().await {
            Ok((socket, remote_addr)) => {
                let shared = shared.clone();
                tokio::spawn(async move {
                    if let Err(e) = timeout(
                        HANDLE_CONNECTION_TIMEOUT,
                        handle_connection(shared, socket, remote_addr),
                    )
                    .await
                    {
                        warn!("{} Failed to handle connection: {}", remote_addr, e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn handle_connection(
    shared: SharedNetworkState,
    stream: tokio::net::TcpStream,
    remote_addr: std::net::SocketAddr,
) {
    if let Err(e) = stream.set_nodelay(true) {
        warn!("Failed to set nodelay on {}: {}", remote_addr, e);
    }
    let io = PacketIo::new(stream);
    if let Err(e) = handle_intent(shared, io, remote_addr).await {
        warn!("Error during handshake with {}: {}", remote_addr, e);
    }
}

