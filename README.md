# nesquic
Like netcat, but using QUIC and written in Rust

1. Build it
```bash
cargo build
```

2. Use it
```bash
cd target/debug

# listen on port 5003/udp
./nesquic -l 5003

# connect to port 5003/udp
./nesquic 127.0.0.1 5003 
```

## Important Notes
1. Connecting end (the one that is not listening) needs to send the first message for flow to be established. Guessing this is because of UDP.
2. `localhost` doesn't work, use `127.0.0.1` instead (maybe fix this in the future)

## Run with debug messages
```bash
cargo build && RUST_LOG=debug ./target/debug/nesquic -l 5003 # listen on port 5003/udp
cargo build && RUST_LOG=debug ./target/debug/nesquic 127.0.0.1 5003 # connect to port 5003/udp
```



