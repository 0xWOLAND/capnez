# `capnez`

This project generates a `.capnp` file at build time from Rust traits/structs, keeping Cap'n Proto projects in pure Rust!

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
- [Hello World Server Example](./example/hello_world/README.md)
- [Serialization Example](./example/serialize/README.md)
- [Sparse Matrix Multiplication Example](./example/sparse_matrix/README.md)
