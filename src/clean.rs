use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn clean(base_dir: &Path) -> Result<()> {
    let extract_dir = base_dir.join("rocky-extracted");
    let output_dir = base_dir.join("output");

    if extract_dir.exists() {
        println!("Removing {}...", extract_dir.display());
        fs::remove_dir_all(&extract_dir)?;
    }

    if output_dir.exists() {
        println!("Removing {}...", output_dir.display());
        fs::remove_dir_all(&output_dir)?;
    }

    println!("Clean complete.");
    Ok(())
}
