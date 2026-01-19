use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn clean(base_dir: &Path) -> Result<()> {
    let downloads_dir = base_dir.join("downloads");
    let output_dir = base_dir.join("output");

    if downloads_dir.exists() {
        println!("Removing {}...", downloads_dir.display());
        fs::remove_dir_all(&downloads_dir)?;
    }

    if output_dir.exists() {
        println!("Removing {}...", output_dir.display());
        fs::remove_dir_all(&output_dir)?;
    }

    println!("Clean complete.");
    Ok(())
}
