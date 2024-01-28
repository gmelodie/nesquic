//! This example demonstrates how to make a QUIC connection that ignores the server certificate.
//!
//! Checkout the `README.md` for guidance.

use std::{
    error::Error,
    io::{stderr, stdin, stdout, BufRead, Write},
    net::SocketAddr,
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
    ///Activate listen mode
    #[clap(short = 'l', long = "listen", action = clap::ArgAction::SetTrue)]
    listen: bool,

    ///IP and Port
    #[clap(value_parser)]
    addr: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    tracing_subscriber::fmt()
        .with_writer(stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Cli::parse();

    // handle ip and port args
    let (ip, port) = match args.addr.len() {
        1 => (None, Some(&args.addr[0])),
        2 => (Some(&args.addr[0]), Some(&args.addr[1])),
        _ => {
            println!("usage: [-l] IP PORT");
            return Ok(());
        }
    };

    debug!(
        "listen:{} ip:{} port:{}",
        args.listen,
        ip.unwrap(),
        port.unwrap()
    );
    match (args.listen, ip, port) {
        // 1. -l ip port
        (true, Some(ip), Some(port)) => {
            let bind_addr = format!("{}:{}", ip, port)
                .parse::<SocketAddr>()
                .expect("unable to parse address");
            let _ = run_server(bind_addr).await;
        }
        // 2. -l port
        (true, None, Some(port)) => {
            let bind_addr = format!("0.0.0.0:{}", port)
                .parse::<SocketAddr>()
                .expect("unable to parse address");
            let _ = run_server(bind_addr).await;
        }
        // 3. ip port (no -l)
        (false, Some(ip), Some(port)) => {
            let server_addr = format!("{}:{}", ip, port)
                .parse::<SocketAddr>()
                .expect("unable to parse address");
            let _ = run_client(server_addr).await;
        }
        _ => {
            println!("usage: [-l] IP PORT");
        }
    }
    Ok(())
}

async fn accept_conn(endpoint: &Endpoint) -> (SendStream, RecvStream) {
    // accept a single connection
    let incoming_conn = endpoint.accept().await.unwrap();
    let conn = incoming_conn.await.unwrap();
    debug!(
        "[server] connection accepted: addr={}",
        conn.remote_address()
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

async fn recv_data(mut recv: RecvStream) -> Result<(), ()> {
    // TODO: use tokio's async io
    let in_order = true;
    let mut stdout = stdout();
    loop {
        match recv.read_chunk(1024 * 1024, in_order).await {
            //TODO: handle ctrl+c as connection closed (aka make ctrl+c send EOF
            Ok(None) => {
                info!("stream was closed by the peer.");
                return Err(());
            }
            Ok(Some(chunk)) => {
                debug!("received {} bytes", chunk.bytes.len());
                let _ = stdout.write_all(&chunk.bytes);
                // continue reading
            }
            Err(e) => {
                // Handle error (e.g., connection error)
                return Err(error!("unexpected error, shutting down {}", e));
            }
        }
    }
}

fn get_input() -> Vec<u8> {
    let stdin = stdin();
    let mut stdin = stdin.lock();
    let buffer = stdin
        .fill_buf()
        .expect("failed to read from stdin")
        .to_vec();
    let length = buffer.len();
    stdin.consume(length);
    buffer
}

async fn send_data(mut send: SendStream) -> Result<(), ()> {
    let mut buffer = vec![0; 64 * 1024];

    // read input from stdin and send it to server until EOF is reached
    loop {
        buffer.clear();
        buffer = get_input();
        if buffer.len() == 0 {
            // EOF reached
            break;
        }
        send.write_all(&buffer).await.unwrap();
        debug!("sent {} bytes", buffer.len());
    }

    // close connection
    info!("[client] closing connection");
    send.finish().await.unwrap();
    Ok(())
}

/// Runs a QUIC server bound to given addr.
async fn run_server(addr: SocketAddr) {
    let (endpoint, _server_cert) = make_server_endpoint(addr).unwrap();
    debug!("[server] running, waiting on connections...");

    // accept connection from client
    loop {
        let (send, recv) = accept_conn(&endpoint).await;
        info!("[server] connection accepted");
        let _ = tokio::spawn(recv_data(recv));
        let _ = send_data(send).await;
        break; // TODO: remove this for multiple connections (maybe a flag?)
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
    info!("[client] connected: addr={}", conn.remote_address());

    // open stream
    let (send, recv) = conn.open_bi().await.unwrap();
    let _ = tokio::spawn(recv_data(recv));
    let _ = send_data(send).await;

    Ok(())
}
