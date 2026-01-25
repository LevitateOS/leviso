//! CLI command handlers.
//!
//! Each submodule handles a specific CLI command:
//! - `build` - Build LevitateOS artifacts
//! - `run` - Run ISO in QEMU (GUI)
//! - `test` - Test ISO boots (headless, automated)
//! - `clean` - Clean build artifacts
//! - `show` - Display information
//! - `download` - Download dependencies
//! - `extract` - Extract archives
//! - `preflight` - Run preflight checks

pub mod build;
pub mod clean;
pub mod download;
pub mod extract;
mod preflight;
mod run;
pub mod show;

pub use build::cmd_build;
pub use clean::cmd_clean;
pub use download::cmd_download;
pub use extract::cmd_extract;
pub use preflight::cmd_preflight;
pub use run::{cmd_run, cmd_test};
pub use show::cmd_show;
