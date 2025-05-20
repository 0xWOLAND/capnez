# Hello World RPC Example

A minimal example demonstrating Cap'n Proto RPC communication between a client and server.

## Running the Example

1. Start the server:
```bash
cargo run -- server localhost:8080
```

2. In another terminal, run the client:
```bash
cargo run -- client localhost:8080 "Your Name"
```

The client will send a greeting request to the server and display the response.

## Project Structure

- `main.rs`: Defines the RPC interface and message types
- `client.rs`: Implements the RPC client
- `server.rs`: Implements the RPC server 