use anyhow::Context;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::status::clientbound::{PongResponse, StatusResponse};
use mcrs_minecraft_protocol::packets::status::serverbound::{PingRequest, StatusRequest};
use mcrs_minecraft_protocol::{Bounded, VarInt};
use serde::Deserialize;
use std::env;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::runtime::Builder;

#[derive(Deserialize)]
struct Status {
    version: Version,
    description: Option<String>,
    players: Players,
}

#[derive(Deserialize)]
struct Version {
    name: String,
    protocol: i32,
}

#[derive(Deserialize)]
struct Players {
    online: u32,
    max: u32,
}

fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    let host = args
        .next()
        .context("usage: mcrs_minecraft_status_ping <host> [port]")?;
    let port = args
        .next()
        .map(|port| port.parse::<u16>())
        .transpose()?
        .unwrap_or(25565);
    anyhow::ensure!(
        args.next().is_none(),
        "usage: mcrs_minecraft_status_ping <host> [port]"
    );
    let runtime = Builder::new_current_thread().enable_all().build()?;
    let result = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(5), ping(&host, port))
            .await
            .context("status ping timed out after 5 seconds")?
    });
    // DNS resolution can outlive the exchange timeout on a blocking worker.
    runtime.shutdown_background();
    result
}

async fn ping(host: &str, port: u16) -> anyhow::Result<()> {
    let stream = TcpStream::connect((host, port))
        .await
        .context("connect to server")?;
    stream.set_nodelay(true)?;
    let mut io = PacketIo::new(stream);
    io.send_packet(&ServerboundHandshake {
        protocol_version: VarInt(VERSION.protocol_version),
        server_address: Bounded(host),
        server_port: port,
        intent: Intent::Status,
    })
    .await?;
    io.send_packet(&StatusRequest).await?;
    let response = io.recv_packet::<StatusResponse>().await?;
    let status: Status = serde_json::from_str(response.json).context("decode status response")?;

    let ping = PingRequest { payload: 1 };
    let started = Instant::now();
    io.send_packet(&ping).await?;
    let pong = io.recv_packet::<PongResponse>().await?;
    let latency = started.elapsed();
    anyhow::ensure!(
        pong.payload == ping.payload,
        "pong payload does not match ping"
    );

    println!(
        "Version: {} (protocol {})",
        status.version.name, status.version.protocol
    );
    println!(
        "Description: {}",
        status.description.as_deref().unwrap_or_default()
    );
    println!("Players: {}/{}", status.players.online, status.players.max);
    println!("Latency: {:.3} ms", latency.as_secs_f64() * 1000.0);
    Ok(())
}
