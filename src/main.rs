//! This example demonstrates how to make a QUIC connection that ignores the server certificate.
//!
//! Checkout the `README.md` for guidance.

use std::{
    error::Error,
    io::{stderr, stdin, stdout, BufRead, Write},
    net::SocketAddr,
    // str::from_utf8,
};

use clap::Parser;

use quinn::{Endpoint, RecvStream, SendStream};

mod util;
use tracing::{debug, error, info};
use tracing_subscriber;
use tracing_subscriber::EnvFilter;
use util::{configure_client, make_server_endpoint};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(short = 'l', action)]
    listen: Option<bool>,

    #[clap(last = true, value_parser)]
    addr: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_writer(stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Cli::parse();

    let mut ip_addr = "0.0.0.0";
    let mut port = "0";
    match (args.listen, args.addr) {
        (true, Some(addr)) => {
            if addr.len() == 2 {
                // 1. -l ip port
                ip_addr = addr[0];
                port = addr[1];
            } else if addr.len() == 1 {
                // 2. -l port
                port = addr[0];
            } else {
                return io::Error::new(io::ErrorKind::Other, "usage: [-l] IP PORT").into());
            }
            let bind_addr = format!("{}:{}", ip_addr, port);
            let _ = run_server(bind_addr).await;

        }
        (false, Some(addr)) => {
            if addr.len() != 2 {
                return io::Error::new(io::ErrorKind::Other, "usage: [-l] IP PORT").into());
            }
            // 3. ip port
            let server_addr = format!("{}:{}", addr[0], addr[1]);
            let _ = run_client(server_addr).await;
        }
        _ => return io::Error::new(io::ErrorKind::Other, "usage: [-l] IP PORT").into());
    }
    Ok(())
}

async fn accept_conn(endpoint: &Endpoint) -> (SendStream, RecvStream) {
    // accept a single connection
    let incoming_conn = endpoint.accept().await.unwrap();
    let conn = incoming_conn.await.unwrap();
    debug!(
        "[server] connection accepted: addr={}",
        conn.remote_addr()
    );
    let stream = match conn.accept_bi().await {
        Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
            panic!("connection closed");
        }
        Err(e) => {
            panic!("{}", e);
        }
        Ok(s) => s,
    };
    debug!("[server] bidirecional stream opened");
    stream
}

/// Runs a QUIC server bound to given addr.
async fn run_server(addr: SocketAddr) {
    // instanciate server
    let (endpoint, _server_cert) = make_server_endpoint(addr).unwrap();
    // accept connection from client
    debug!("[server] running, waiting on connections...");

    loop {
        let (mut _send, mut recv) = accept_conn(&endpoint).await;
        debug!("[server] connection accepted");
        let in_order = true;
        loop {
            match recv.read_chunk(1024 * 1024, in_order).await {
                //TODO: handle ctrl+c as connection closed (aka make ctrl+c send EOF
                Ok(None) => {
                    info!("stream was closed by the peer.");
                    break;
                }
                Ok(Some(chunk)) => {
                    // return Ok(Some(chunk.bytes));
                    let _ = stdout().write_all(&chunk.bytes);
                }
                Err(e) => {
                    // Handle error (e.g., connection error)
                    error!("Unexpected error, shutting down {}", e);
                    return;
                }
            }
        }
    }
}

async fn run_client(server_addr: SocketAddr) -> Result<(), Box<dyn Error>> {
    let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())?;
    endpoint.set_default_client_config(configure_client());

    // connect to server
    let conn = endpoint
        .connect(server_addr, "127.0.0.1")
        .unwrap()
        .await
        .expect("could not connect to server");
    info!("[client] connected: addr={}", conn.remote_addr());

    // open stream
    let (mut send, mut _recv) = conn.open_bi().await.unwrap();

    // instanciate stdin reader
    let stdin = stdin();
    let mut stdin = stdin.lock();
    let mut buffer = vec![0; 64 * 1024];

    // read input from stdin and send it to server until EOF is reached
    loop {
        buffer.clear();
        let buffer = stdin.fill_buf().expect("failed to read from stdin");
        if buffer.len() == 0 {
            // EOF reached
            break;
        }
        send.write_all(&buffer).await.unwrap();
        let length = buffer.len();
        debug!("sent {} bytes", length);
        stdin.consume(length);
    }

    // close connection
    info!("[client] closing connection");
    send.finish().await.unwrap();

    drop(conn);

    Ok(())
}
