use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

pub fn test_qemu(base_dir: &Path, gui: bool) -> Result<()> {
    let output_dir = base_dir.join("output");
    let iso_path = output_dir.join("leviso.iso");

    if !iso_path.exists() {
        bail!(
            "ISO not found at {}. Run 'leviso build' or 'leviso iso' first.",
            iso_path.display()
        );
    }

    println!("Starting QEMU with {}...", iso_path.display());

    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.args([
        "-cpu",
        "Skylake-Client",
        "-cdrom",
        iso_path.to_str().unwrap(),
        "-m",
        "512M",
    ]);

    if gui {
        println!("Running with GUI window");
    } else {
        println!("Press Ctrl+A, X to exit QEMU\n");
        cmd.args(["-nographic", "-serial", "mon:stdio"]);
    }

    let status = cmd
        .status()
        .context("Failed to run qemu-system-x86_64. Is QEMU installed?")?;

    if !status.success() {
        bail!("QEMU exited with status: {}", status);
    }

    Ok(())
}
