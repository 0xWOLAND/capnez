# Capnez Serialization Example

A minimal example demonstrating serialization using Cap'n Proto and Serde in Rust.

## What it does

This example shows how to:
- Define a struct with `#[capnp]` and `#[derive(Serialize, Deserialize)]`
- Serialize a struct to Cap'n Proto format
- Deserialize Cap'n Proto bytes back into a struct