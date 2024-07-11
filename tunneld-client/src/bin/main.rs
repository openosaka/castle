use bytes::Bytes;
use clap::{Parser, Subcommand};
use std::net::{SocketAddr, ToSocketAddrs};
use tokio::{net::lookup_host, signal};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tunneld_pkg::{otel::setup_logging, shutdown};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, default_value = "127.0.0.1:6610")]
    server_addr: SocketAddr,
}

#[derive(Subcommand)]
enum Commands {
    Tcp {
        #[clap(index = 1)]
        port: u16,
        #[arg(long, required = true)]
        remote_port: u16,
        #[arg(
            long,
            default_value = "127.0.0.1",
            help = "Local address to bind to, e.g localhost, example.com"
        )]
        local_addr: String,
        #[arg(long, default_value = "false")]
        random_remote_port: bool,
    },
    Http {
        #[clap(index = 1)]
        port: u16,
        #[arg(long)]
        remote_port: Option<u16>,
        #[arg(long)]
        subdomain: Option<String>,
        #[arg(long)]
        domain: Option<String>,
        #[arg(
            long,
            default_value = "127.0.0.1",
            help = "Local address to bind to, e.g localhost, example.com"
        )]
        local_addr: String,
        #[arg(long, default_value = "false")]
        random_remote_port: bool,
    },
    Udp {
        #[clap(index = 1)]
        port: u16,
        #[arg(long, required = true)]
        remote_port: u16,
        #[arg(
            long,
            default_value = "127.0.0.1",
            help = "Local address to bind to, e.g localhost, example.com"
        )]
        local_addr: String,
        #[arg(long, default_value = "false")]
        random_remote_port: bool,
    },
}

const TUNNEL_NAME: &str = "tunneld-client";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 6670 is the default tokio console server port of the client,
    // use `TOKIO_CONSOLE_BIND=127.0.0.1:6669` to change it.
    setup_logging(6670);

    let args = Args::parse();

    let cancel_w = CancellationToken::new();
    let cancel = cancel_w.clone();

    tokio::spawn(async move {
        if let Err(e) = signal::ctrl_c().await {
            // Something really weird happened. So just panic
            panic!("Failed to listen for the ctrl-c signal: {:?}", e);
        }
        info!("Received ctrl-c signal. Shutting down...");
        cancel_w.cancel();
    });

    let mut client = tunneld_client::Client::new(&args.server_addr).unwrap();

    match args.command {
        Commands::Tcp {
            port,
            remote_port,
            local_addr,
            random_remote_port,
        } => {
            let local_endpoint = parse_socket_addr(&local_addr, port).await?;
            client.add_tcp_tunnel(
                TUNNEL_NAME.to_string(),
                local_endpoint,
                remote_port,
                random_remote_port,
            );
        }
        Commands::Udp {
            port,
            remote_port,
            local_addr,
            random_remote_port,
        } => {
            let local_endpoint = parse_socket_addr(&local_addr, port).await?;
            client.add_udp_tunnel(
                TUNNEL_NAME.to_string(),
                local_endpoint,
                remote_port,
                random_remote_port,
            );
        }
        Commands::Http {
            port,
            local_addr,
            remote_port,
            subdomain,
            domain,
            random_remote_port,
        } => {
            let local_endpoint = parse_socket_addr(&local_addr, port).await?;
            client.add_http_tunnel(
                TUNNEL_NAME.to_string(),
                local_endpoint,
                remote_port.unwrap_or(0),
                Bytes::from(subdomain.unwrap_or_default()),
                Bytes::from(domain.unwrap_or_default()),
                random_remote_port,
            );
        }
    }

    client
        .run(shutdown::ShutdownListener::from_cancellation(cancel))
        .await
}

async fn parse_socket_addr(local_addr: &str, port: u16) -> anyhow::Result<SocketAddr> {
    let addr = format!("{}:{}", local_addr, port);
    let mut addrs = addr.to_socket_addrs()?;
    if addrs.len() == 1 {
        return Ok(addrs.next().unwrap());
    }
    let ips = lookup_host(addr).await?.collect::<Vec<_>>();
    if !ips.is_empty() {
        info!(port = port, ips = ?ips, "dns parsed",);

        let mut ip = ips[0];
        ip.set_port(port);
        return Ok(ip);
    }

    Err(anyhow::anyhow!("Invalid address"))
}
