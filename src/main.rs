//! This example demonstrates how to make a QUIC connection that ignores the server certificate.
//!
//! Checkout the `README.md` for guidance.

use std::{
    error::Error,
    io::{stdin, stdout, BufRead, BufReader, Write},
    net::SocketAddr,
    str::from_utf8,
};

use quinn::{Endpoint, RecvStream, SendStream};

mod util;
use tracing::debug;
use tracing_subscriber;
use util::{configure_client, make_server_endpoint};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    // server and client are running on the same thread asynchronously
    let addr = "127.0.0.1:5000".parse().unwrap();
    tokio::spawn(run_server(addr));
    run_client(addr).await?;
    Ok(())
}

async fn recv_until(recv: &mut quinn::RecvStream, delim: u8) -> Vec<u8> {
    //TODO: implement stopping in delim logic
    let mut buffer = vec![0; 1024];
    loop {
        let bytes_read = recv.read(&mut buffer).await.unwrap();
        if bytes_read == Some(0) || bytes_read == None {
            continue;
        }
        debug!("read '{}'", from_utf8(&buffer).unwrap());
        if buffer.iter().find(|&&x| x == delim).is_some() {
            // if found delim
            return buffer;
        }
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
    let (mut _send, mut recv) = accept_conn(endpoint).await;
    loop {
        // recv one line from client
        let msg = recv_until(&mut recv, b'\n').await;
        // print message to screen
        let _ = stdout().write_all(&msg);
    }
}

async fn run_client(server_addr: SocketAddr) -> Result<(), Box<dyn Error>> {
    let mut endpoint = Endpoint::client("127.0.0.1:0".parse().unwrap())?;
    endpoint.set_default_client_config(configure_client());

    // connect to server
    let conn = endpoint
        .connect(server_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    println!("[client] connected: addr={}", conn.remote_address());

    // open stream
    let (mut send, mut _recv) = conn.open_bi().await.unwrap();

    // instanciate stdin reader
    let mut reader = BufReader::new(stdin());
    let mut buffer = vec![];

    loop {
        let _ = reader.read_until(b'\n', &mut buffer);
        send.write_all(&buffer).await.unwrap();
        buffer.clear();
    }
    // TODO: read input from stdin

    // send data that was read
    send.finish().await.unwrap();

    drop(conn);
    // Make sure the server has a chance to clean up
    endpoint.wait_idle().await;

    Ok(())
}
