# nesquic
Like netcat, but using QUIC and written in Rust

Run for developing (with debug messages)
```bash
# run "server" aka listen
cargo build && RUST_LOG=debug ./target/debug/nesquic server 0.0.0.0:5003

# run "client" aka initiator
cargo build && RUST_LOG=debug ./target/debug/nesquic client
```
