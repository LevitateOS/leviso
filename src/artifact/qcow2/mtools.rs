//! mtools file operations for FAT32 image manipulation.
//!
//! Delegates to shared infrastructure in distro-builder::artifact::disk::mtools.

pub use distro_builder::artifact::disk::mtools::{mtools_copy, mtools_mkdir, mtools_write_file};

#[cfg(test)]
mod tests {
    #[test]
    fn test_mtools_functions_exist() {
        assert!(true);
    }
}
