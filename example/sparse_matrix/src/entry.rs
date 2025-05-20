use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixEntry {
    pub row: u32,
    pub col: u32,
    pub value: f64,
}
