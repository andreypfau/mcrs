use crate::SharedNetworkState;
use crate::connect::{AcceptGate, HANDLE_CONNECTION_TIMEOUT};
use crate::intent::handle_intent;
use crate::packet_io::{ByteStream, PacketIo};
use log::{info, warn};
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::timeout;
use wtransport::endpoint::IncomingSession;
use wtransport::endpoint::endpoint_side::Server;
use wtransport::stream::BiStream;
use wtransport::{Connection, Endpoint, Identity, RecvStream, SendStream, ServerConfig};

/// The HTTP/3 session owns the CONNECT stream the WebTransport streams ride on,
/// so it has to outlive them; carrying it alongside the stream is what keeps it
/// from being dropped when the handshake task returns.
pub struct SessionStream<S> {
    stream: S,
    session: Connection,
}

impl<S: AsyncRead + Unpin> AsyncRead for SessionStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for SessionStream<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

impl ByteStream for SessionStream<BiStream> {
    type Reader = SessionStream<RecvStream>;
    type Writer = SendStream;

    fn split_stream(self) -> (Self::Reader, Self::Writer) {
        let (send, recv) = self.stream.split();
        (
            SessionStream {
                stream: recv,
                session: self.session,
            },
            send,
        )
    }
}

pub(crate) fn bind(address: SocketAddr) -> anyhow::Result<(Endpoint<Server>, SocketAddr, [u8; 32])> {
    let identity = Identity::self_signed(["localhost", "127.0.0.1", "::1"])?;
    let certificate_hash = *identity.certificate_chain().as_slice()[0].hash().as_ref();
    let config = ServerConfig::builder()
        .with_bind_address(address)
        .with_identity(identity)
        .build();
    let endpoint = Endpoint::server(config)?;
    let bound = endpoint.local_addr()?;
    let hex: String = certificate_hash.iter().map(|b| format!("{b:02x}")).collect();
    info!("WebTransport listening on {bound} (certificate hash {hex})");
    Ok((endpoint, bound, certificate_hash))
}

pub(crate) async fn start_accept_loop(shared: SharedNetworkState, endpoint: Endpoint<Server>) {
    let mut gate = AcceptGate::new();

    loop {
        let incoming = endpoint.accept().await;
        let remote_addr = incoming.remote_address();
        let Some(guard) = gate.admit(remote_addr.ip()) else {
            incoming.refuse();
            continue;
        };
        let shared = shared.clone();
        tokio::spawn(async move {
            let _guard = guard;
            match timeout(
                HANDLE_CONNECTION_TIMEOUT,
                handle_session(shared, incoming, remote_addr),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => warn!("{} WebTransport session failed: {}", remote_addr, e),
                Err(e) => warn!("{} WebTransport handshake timed out: {}", remote_addr, e),
            }
        });
    }
}

async fn handle_session(
    shared: SharedNetworkState,
    incoming: IncomingSession,
    remote_addr: SocketAddr,
) -> anyhow::Result<()> {
    let session = incoming.await?.accept().await?;
    let stream = BiStream::join(session.accept_bi().await?);
    let io = PacketIo::new(SessionStream { stream, session });
    handle_intent(shared, io, remote_addr).await
}
