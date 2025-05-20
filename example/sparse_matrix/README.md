# Sparse Matrix Example

A simple example demonstrating sparse matrix multiplication with Cap'n Proto serialization.

## What it does

- Implements sparse matrix multiplication
- Uses Cap'n Proto for efficient serialization/deserialization
- Demonstrates how to use both Serde and Cap'n Proto attributes together

## Running the example

```bash
cargo run
```

The program will:
1. Create two sparse matrices
2. Multiply them
3. Serialize the result using Cap'n Proto
4. Verify the serialization by deserializing and comparing