//! This example demonstrates how to make a QUIC connection that ignores the server certificate.
//!
//! Checkout the `README.md` for guidance.

use std::{
    error::Error,
    io::{stderr, stdin, stdout, BufRead, Write},
    net::SocketAddr,
    // str::from_utf8,
};

use clap::{App, Arg, SubCommand};

use quinn::{Endpoint, RecvStream, SendStream};

mod util;
use tracing::{debug, error, info};
use tracing_subscriber;
use tracing_subscriber::EnvFilter;
use util::{configure_client, make_server_endpoint};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_writer(stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let matches = App::new("Nesquic")
        .subcommand(
            SubCommand::with_name("client")
                .about("Runs the client (sender)")
                .arg(
                    Arg::with_name("server_address")
                        .help("The server address to connect to")
                        .default_value("127.0.0.1:5003")
                        .index(1),
                ),
        )
        .subcommand(
            SubCommand::with_name("server")
                .about("Runs the server (receiver)")
                .arg(
                    Arg::with_name("bind_address")
                        .help("The address server will listen on")
                        .default_value("127.0.0.1:5003")
                        .index(1),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("client", client_matches)) => {
            let server_address = client_matches.value_of("server_address").unwrap();
            let _ = run_client(server_address.parse().unwrap()).await;
        }
        Some(("server", server_matches)) => {
            let bind_address = server_matches.value_of("bind_address").unwrap();
            let _ = run_server(bind_address.parse().unwrap()).await;
        }
        _ => unreachable!(), // If no subcommand was used it'll match the empty tuple
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
    loop {
        match recv.read_chunk(1024 * 1024, in_order).await {
            //TODO: handle ctrl+c as connection closed (aka make ctrl+c send EOF
            Ok(None) => {
                info!("stream was closed by the peer.");
                return Err(());
            }
            Ok(Some(chunk)) => {
                debug!("received {} bytes", chunk.bytes.len());
                // let stdout = stdout();
                // let mut stdout = stdout.lock();
                let _ = stdout().write_all(&chunk.bytes);
                // continue reading
            }
            Err(e) => {
                // Handle error (e.g., connection error)
                error!("unexpected error, shutting down {}", e);
                return Err(());
            }
        }
    }
}

async fn send_data(mut send: SendStream) -> Result<(), ()> {
    // TODO: use tokio's async io
    // instanciate stdin reader
    // let stdin = stdin();
    // let mut stdin = stdin.lock();
    let mut buffer = vec![0; 64 * 1024];

    // read input from stdin and send it to server until EOF is reached
    loop {
        buffer.clear();
        let buffer = stdin().fill_buf().expect("failed to read from stdin");
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
    Err(()) // TODO: this should be an OK, but an OK doesnt stop the other recv
}

/// Runs a QUIC server bound to given addr.
async fn run_server(addr: SocketAddr) {
    // instanciate server
    let (endpoint, _server_cert) = make_server_endpoint(addr).unwrap();
    // accept connection from client
    debug!("[server] running, waiting on connections...");

    loop {
        let (send, recv) = accept_conn(&endpoint).await;
        debug!("[server] connection accepted");
        let _ = tokio::try_join!(recv_data(recv), send_data(send));
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
    let _ = tokio::try_join!(send_data(send), recv_data(recv));

    Ok(())
}
