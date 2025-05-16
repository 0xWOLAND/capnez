use anyhow::{Context, Result};
use std::path::PathBuf;

fn main() -> Result<()> {
    let input = PathBuf::from("../capnez/src");
    let output = PathBuf::from("../target/generated");
    
    println!("Generating Cap'n Proto schema from {}", input.display());
    println!("Output will be written to {}", output.display());
    
    codegen::generate_schema(&input, &output)
        .context("Failed to generate schema")?;
        
    println!("Successfully generated schema in {}", output.display());
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_generate_schema() -> Result<()> {
        let input = PathBuf::from("../capnez/src");
        let output = PathBuf::from("../target/generated");
        
        // Clean up any existing generated files
        if output.exists() {
            fs::remove_dir_all(&output)?;
        }
        
        // Generate the schema
        codegen::generate_schema(&input, &output)?;
        
        // Verify that files were generated
        assert!(output.exists(), "Output directory was not created");
        assert!(output.join("schema_capnp.rs").exists(), "Generated Rust file not found");
        
        Ok(())
    }
} 