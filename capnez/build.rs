use anyhow::Result;

fn main() -> Result<()> {
    codegen::generate_schema()?;
    println!("cargo:rerun-if-changed=src");
    
    Ok(())
}
