mod entry;
mod matrix;
mod multiply;

use entry::MatrixEntry;
use matrix::SparseMatrix;
use multiply::multiply;
use std::fs::File;
use capnp::serialize;
use capnez_codegen::capnp_include;
use capnez_macros::capnp;
use std::error::Error;

#[capnp]
struct SparseMatrixData {
    rows: u32,
    cols: u32,
    values: Vec<MatrixEntry>,
}

capnp_include!();

fn main() -> Result<(), Box<dyn Error>> {
    // Create and fill matrices in one go using iterators
    let a = [(0,0,1.0), (0,2,2.0), (1,1,3.0), (2,0,4.0), (2,3,5.0)].iter()
        .fold(SparseMatrix::new(3, 4), |mut m, &(r,c,v)| { m.insert(r,c,v); m });
    let b = [(0,0,1.0), (1,1,2.0), (2,0,3.0), (3,1,4.0)].iter()
        .fold(SparseMatrix::new(4, 2), |mut m, &(r,c,v)| { m.insert(r,c,v); m });

    let result = multiply(&a, &b).expect("Matrix dimensions should be compatible");

    // Serialize to file
    let mut msg = capnp::message::Builder::new_default();
    let mut builder = msg.init_root::<schema_capnp::sparse_matrix::Builder>();
    builder.set_rows(result.rows);
    builder.set_cols(result.cols);
    
    let mut values = builder.init_values(result.values.len() as u32);
    for (i, e) in result.values.iter().enumerate() {
        let mut entry = values.reborrow().get(i as u32);
        entry.set_row(e.row);
        entry.set_col(e.col);
        entry.set_value(e.value);
    }

    let path = format!("{}/target/result.bin", env!("OUT_DIR"));
    std::fs::create_dir_all(format!("{}/target", env!("OUT_DIR")))?;
    let mut file = File::create(&path)?;
    serialize::write_message(&mut file, &msg)?;
    println!("\nSerialized to {}", path);

    // Verify serialization
    let mut file = File::open(&path)?;
    let message_reader = serialize::read_message(&mut file, capnp::message::ReaderOptions::new())?;
    let reader = message_reader.get_root::<schema_capnp::sparse_matrix::Reader>()?;
    
    assert_eq!(reader.get_rows(), result.rows);
    assert_eq!(reader.get_cols(), result.cols);
    
    let values = reader.get_values()?;
    for (i, e) in values.iter().enumerate() {
        assert_eq!(e.get_row(), result.values[i].row);
        assert_eq!(e.get_col(), result.values[i].col);
        assert!((e.get_value() - result.values[i].value).abs() < 1e-6);
    }
    println!("Deserialization passed!");
    Ok(())
} 