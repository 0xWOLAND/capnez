use crate::matrix::SparseMatrix;
use std::collections::HashMap;

pub fn multiply(a: &SparseMatrix, b: &SparseMatrix) -> Option<SparseMatrix> {
    if a.cols != b.rows {
        return None;
    }

    // Create a map of column indices to values for each row in matrix B
    let mut b_cols: Vec<HashMap<u32, f64>> = vec![HashMap::new(); b.rows as usize];
    for entry in &b.values {
        b_cols[entry.row as usize].insert(entry.col, entry.value);
    }

    let mut result = SparseMatrix::new(a.rows, b.cols);
    let mut row_values: HashMap<u32, f64> = HashMap::new();

    // For each row in matrix A
    for a_row in 0..a.rows {
        row_values.clear();

        // For each non-zero element in this row of A
        for a_entry in a.values.iter().filter(|e| e.row == a_row) {
            let a_col = a_entry.col;
            let a_val = a_entry.value;

            // Multiply with corresponding elements in B
            if let Some(b_row_values) = b_cols.get(a_col as usize) {
                for (&b_col, &b_val) in b_row_values {
                    let product = a_val * b_val;
                    *row_values.entry(b_col).or_insert(0.0) += product;
                }
            }
        }

        // Add non-zero results to the output matrix
        for (col, value) in row_values.drain() {
            if value != 0.0 {
                result.insert(a_row, col, value);
            }
        }
    }

    Some(result)
} 