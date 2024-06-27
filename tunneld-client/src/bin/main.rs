use std::net::SocketAddr;

use clap::{Parser, Subcommand};
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::info;

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
    },
    Http {
        #[arg(long)]
        remote_port: u16,
        #[arg(long)]
        subdomain: String,
        #[arg(long)]
        domain: String,
    },
}

const TUNNEL_NAME: &str = "tunneld-client";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

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
        Commands::Tcp { port, remote_port } => {
            client.add_tcp_tunnel(TUNNEL_NAME.to_string(), remote_port, port);
        }
        Commands::Http {
            remote_port,
            subdomain,
            domain,
        } => {
            client.add_http_tunnel(remote_port, &subdomain, &domain);
        }
    }

    if let Err(err) = client.run(cancel).await {
        eprintln!("server error: {:?}", err);
    }

    println!("Server address: {:?}", &args.server_addr);
}
