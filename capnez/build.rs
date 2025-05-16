use anyhow::{Context, Result};
use std::path::PathBuf;
use std::env;

fn main() -> Result<()> {
    // Get the absolute path to the crate root
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    
    let input = manifest_dir.join("src");
    let output = out_dir.join("generated");
    
    println!("cargo:warning=Generating Cap'n Proto schema from {}", input.display());
    println!("cargo:warning=Output will be written to {}", output.display());
    
    codegen::generate_schema(&input, &output)
        .context("Failed to generate schema")?;
        
    println!("cargo:warning=Successfully generated schema in {}", output.display());
    
    // Tell cargo to rerun this if any source files change
    println!("cargo:rerun-if-changed=src");
    
    Ok(())
}
