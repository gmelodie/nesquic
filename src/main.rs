//! This example demonstrates how to make a QUIC connection that ignores the server certificate.
//!
//! Checkout the `README.md` for guidance.

use std::{
    error::Error,
    io::{stdin, stdout, BufRead, BufReader, Write},
    net::SocketAddr,
    // str::from_utf8,
};

use clap::{App, Arg, SubCommand};

use quinn::{Endpoint, RecvStream, SendStream};

mod util;
use tracing::{debug, info};
use tracing_subscriber;
use util::{configure_client, make_server_endpoint};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
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

async fn recv_until(
    recv: &mut quinn::RecvStream,
    delim: u8,
) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    let mut buffer = vec![0; 1024];
    loop {
        // let bytes_read = recv.read(&mut buffer).await.unwrap();

        match recv.read(&mut buffer).await {
            Ok(None) => {
                info!("stream was closed by the peer.");
                return Ok(None);
            }
            Ok(Some(_bytes_read)) => {
                // Successfully read _bytes_read bytes
                if buffer.iter().find(|&&x| x == delim).is_some() {
                    // if found delim
                    return Ok(Some(buffer));
                }
                // Handle data in buffer
            }
            Err(e) => {
                // Handle error (e.g., connection error)
                return Err(Box::new(e));
            }
        }
        // if bytes_read == Some(0) || bytes_read == None {
        //     continue;
        // }
        // debug!("read '{}'", from_utf8(&buffer).unwrap());
        // if buffer.iter().find(|&&x| x == delim).is_some() {
        //     // if found delim
        //     return buffer;
        // }
    }
}

async fn accept_conn(endpoint: Endpoint) -> (SendStream, RecvStream) {
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

/// Runs a QUIC server bound to given address.
async fn run_server(addr: SocketAddr) {
    // instanciate server
    let (endpoint, _server_cert) = make_server_endpoint(addr).unwrap();
    // accept connection from client
    debug!("[server] running, waiting on connections...");
    let (mut _send, mut recv) = accept_conn(endpoint).await;
    debug!("[server] connection accepted");
    loop {
        // recv one line from client
        // let msg = recv_until(&mut recv, b'\n').await;
        let _ = match recv_until(&mut recv, b'\n').await {
            Ok(Some(msg)) => stdout().write_all(&msg),
            Ok(None) => break,
            Err(e) => panic!("{}", e),
        };
    }
}

async fn run_client(server_addr: SocketAddr) -> Result<(), Box<dyn Error>> {
    let mut endpoint = Endpoint::client("127.0.0.1:0".parse().unwrap())?;
    endpoint.set_default_client_config(configure_client());

    // connect to server
    let conn = endpoint
        .connect(server_addr, "127.0.0.1")
        .unwrap()
        .await
        .expect("could not connect to server");
    info!("[client] connected: addr={}", conn.remote_address());

    // open stream
    let (mut send, mut _recv) = conn.open_bi().await.unwrap();

    // instanciate stdin reader
    let mut reader = BufReader::new(stdin());
    let mut buffer = vec![];

    // read input from stdin and send it to server until EOF is reached
    loop {
        buffer.clear();
        let bytes_read = reader
            .read_until(b'\n', &mut buffer)
            .expect("failed to read from stdin");
        if bytes_read == 0 {
            // EOF reached
            break;
        }
        send.write_all(&buffer).await.unwrap();
        buffer.clear();
    }

    // close connection
    info!("[client] closing connection");
    send.finish().await.unwrap();

    drop(conn);

    Ok(())
}
