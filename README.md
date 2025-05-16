# `capnez`

This project generates a `.capnp` file at compile time from Rust traits/structs, keeping Cap'n Proto projects in pure Rust!

## Usage

Add the following to your `Cargo.toml`:

```toml
[dependencies]
capnez-macros = { git = "https://github.com/0xWOLAND/capnez" }
capnp = "0.21.0"
capnp-rpc = "0.21.0"

[build-dependencies]
capnez-codegen = { git = "https://github.com/0xWOLAND/capnez" }
```

Then in your Rust code, define your Cap'n Proto interface using Rust types:

```rust
use capnez_macros::capnp;

// Define request/response structs
#[capnp]
struct HelloRequest {
    name: String,
}

#[capnp]
struct HelloReply {
    message: String,
}

// Define your service interface
#[capnp]
trait HelloWorld {
    fn sayHello(request: HelloRequest) -> HelloReply;
}
```

The `#[capnp]` attribute macro will identify the structs/traits that are used in the networked setting. Then, use 

```rust
capnez_codegen::generate_schema().expect("Failed to generate schema");
```

To generate the schema at build time.


## Example

See the `example` directory for a complete client-server implementation using this library.

Run: 
```shell
cargo run -- server 127.0.0.1:8000 # In terminal 1
cargo run -- client 127.0.0.1:8000 "Bhargav" # In terminal 2
```

Which should output 
```
Running `.../example client '127.0.0.1:8000' Bhargav`
received: Hello, Bhargav!
```
