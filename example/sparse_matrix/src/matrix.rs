use serde::{Serialize, Deserialize};
use crate::entry::MatrixEntry;

#[capnez_macros::capnp]
#[derive(Debug, Clone)]
pub struct SparseMatrix {
    pub rows: u32,
    pub cols: u32,
    pub values: Vec<MatrixEntry>,
}

impl SparseMatrix {
    pub fn new(rows: u32, cols: u32) -> Self {
        Self {
            rows,
            cols,
            values: Vec::new(),
        }
    }

    pub fn insert(&mut self, row: u32, col: u32, value: f64) {
        if row >= self.rows || col >= self.cols {
            panic!("Index out of bounds");
        }
        self.values.push(MatrixEntry { row, col, value });
    }
} 