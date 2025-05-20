# `capnez`

This project generates a `.capnp` file at build time from Rust traits/structs, keeping Cap'n Proto projects in pure Rust!

## Features
- Generate Cap'n Proto schemas from Rust structs/traits
- Support for primitive types, lists, optionals, and nested structs
- `serde` integration for serialization

## Dependencies

Install the `capnp` command line tool from [here](https://capnproto.org/install.html) 

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
use capnez::capnp_include;

capnp_include!();

#[derive(Capnp, Serialize, Deserialize)]
struct Person {
    name: String,
    age: u32,
    email: String,
}
```

The `#[capnp]` attribute macro will identify the structs/traits that are used in the networked setting. Then, use 

```rust
capnez_codegen::generate_schema().expect("Failed to generate schema");
```

To generate the schema at build time.

## Examples

- [`hello_world`](./example/hello_world/README.md)
- [`serialize`](./example/serialize/README.md)
- [`sparse_matrix`](./example/sparse_matrix/README.md)
