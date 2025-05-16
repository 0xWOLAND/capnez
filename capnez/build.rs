use anyhow::{Context, Result};
use std::path::PathBuf;

fn main() -> Result<()> {
    let input = PathBuf::from("src");
    let output = PathBuf::from("src/generated");
    
    println!("cargo:warning=Generating Cap'n Proto schema from {}", input.display());
    println!("cargo:warning=Output will be written to {}", output.display());
    
    codegen::generate_schema(&input, &output)
        .context("Failed to generate schema")?;
        
    println!("cargo:warning=Successfully generated schema in {}", output.display());
    
    // Tell cargo to rerun this if any source files change
    println!("cargo:rerun-if-changed=src");
    
    Ok(())
}
