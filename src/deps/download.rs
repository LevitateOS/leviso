//! Centralized download functionality.
//!
//! All downloads (HTTP, BitTorrent, Git) go through this module for consistent:
//! - Error handling with full context
//! - Retry logic for transient failures
//! - Progress reporting
//! - Resume support where applicable

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

// Re-export for convenience
pub use checksum::verify_sha256;

/// Download configuration options.
#[derive(Debug, Clone)]
pub struct DownloadOptions {
    /// Request timeout (default: 30 seconds for metadata, none for large files)
    pub timeout: Option<Duration>,
    /// Number of retry attempts for transient failures (default: 3)
    pub retries: u32,
    /// Delay between retries (default: 2 seconds, doubles each retry)
    pub retry_delay: Duration,
    /// Whether to show progress (default: true)
    pub show_progress: bool,
    /// Expected file size in bytes (for progress calculation)
    pub expected_size: Option<u64>,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            timeout: None, // No timeout for large downloads
            retries: 3,
            retry_delay: Duration::from_secs(2),
            show_progress: true,
            expected_size: None,
        }
    }
}

impl DownloadOptions {
    /// Quick metadata fetch (short timeout, no progress)
    #[allow(dead_code)]
    pub fn metadata() -> Self {
        Self {
            timeout: Some(Duration::from_secs(30)),
            retries: 3,
            retry_delay: Duration::from_secs(1),
            show_progress: false,
            expected_size: None,
        }
    }

    /// Large file download with expected size
    pub fn large_file(size_bytes: u64) -> Self {
        Self {
            timeout: None,
            retries: 3,
            retry_delay: Duration::from_secs(5),
            show_progress: true,
            expected_size: Some(size_bytes),
        }
    }
}

/// Progress information passed to callbacks.
#[derive(Debug, Clone)]
pub struct Progress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub percent: Option<u8>,
    #[allow(dead_code)]
    pub speed_bps: Option<u64>,
}

impl Progress {
    fn new(downloaded: u64, total: Option<u64>) -> Self {
        let percent = total.map(|t| {
            if t > 0 {
                ((downloaded * 100) / t) as u8
            } else {
                0
            }
        });
        Self {
            downloaded,
            total,
            percent,
            speed_bps: None,
        }
    }

    /// Format as human-readable string
    pub fn display(&self) -> String {
        let downloaded_mb = self.downloaded as f64 / (1024.0 * 1024.0);
        match (self.total, self.percent) {
            (Some(total), Some(pct)) => {
                let total_mb = total as f64 / (1024.0 * 1024.0);
                format!("{:.1}/{:.1} MB ({}%)", downloaded_mb, total_mb, pct)
            }
            _ => format!("{:.1} MB", downloaded_mb),
        }
    }
}

// =============================================================================
// HTTP Downloads
// =============================================================================

/// Download a file via HTTP with resume support.
///
/// # Errors
/// Returns detailed error with URL, HTTP status, and retry information.
pub async fn http(url: &str, dest: &Path, options: &DownloadOptions) -> Result<()> {
    

    let client = reqwest::Client::builder()
        .user_agent("leviso/0.1")
        .build()
        .context("Failed to create HTTP client")?;

    let mut last_error = None;
    let mut attempt = 0;

    while attempt <= options.retries {
        if attempt > 0 {
            let delay = options.retry_delay * (1 << (attempt - 1).min(4)); // Exponential backoff, max 16x
            if options.show_progress {
                println!(
                    "    Retry {}/{} in {:?}...",
                    attempt, options.retries, delay
                );
            }
            tokio::time::sleep(delay).await;
        }
        attempt += 1;

        match http_attempt(&client, url, dest, options).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                // Check if error is retryable
                let is_retryable = is_retryable_error(&e);
                if !is_retryable || attempt > options.retries {
                    return Err(e);
                }
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Download failed after {} retries", options.retries)))
}

/// Single HTTP download attempt.
async fn http_attempt(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    options: &DownloadOptions,
) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    // Check for partial download to resume
    let mut start_byte = if dest.exists() {
        std::fs::metadata(dest)
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        0
    };

    // Build request
    let mut request = client.get(url);
    if let Some(timeout) = options.timeout {
        request = request.timeout(timeout);
    }
    let requested_resume = start_byte > 0;
    if requested_resume {
        request = request.header("Range", format!("bytes={}-", start_byte));
        if options.show_progress {
            println!("    Resuming from {} bytes", start_byte);
        }
    }

    // Send request
    let response = request
        .send()
        .await
        .with_context(|| format!("HTTP request failed: {}", url))?;

    // Check status
    let status = response.status();
    if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
        bail!(
            "HTTP {} for {}: {}",
            status.as_u16(),
            url,
            status.canonical_reason().unwrap_or("Unknown error")
        );
    }

    // CRITICAL: If we requested resume but got 200 OK instead of 206 Partial Content,
    // the server doesn't support resume. We must start fresh to avoid corruption.
    if requested_resume && status == reqwest::StatusCode::OK {
        if options.show_progress {
            println!("    Server doesn't support resume, starting fresh");
        }
        start_byte = 0;
        // Will create fresh file below
    }

    // Get content length
    let content_length = response.content_length();
    let total_size = content_length
        .map(|len| len + start_byte)
        .or(options.expected_size);

    // Open file
    let file = if start_byte > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT {
        // Only append if server confirmed partial content
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(dest)
            .await
            .with_context(|| format!("Failed to open {} for append", dest.display()))?
    } else {
        // Fresh download - ensure parent directory exists
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }
        tokio::fs::File::create(dest)
            .await
            .with_context(|| format!("Failed to create {}", dest.display()))?
    };
    let mut writer = tokio::io::BufWriter::new(file);

    // Stream response
    let mut downloaded = start_byte;
    let mut last_percent = 0u8;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("Failed to read chunk from {}", url))?;
        writer
            .write_all(&chunk)
            .await
            .with_context(|| format!("Failed to write to {}", dest.display()))?;
        downloaded += chunk.len() as u64;

        // Progress
        if options.show_progress {
            let progress = Progress::new(downloaded, total_size);
            if let Some(pct) = progress.percent {
                if pct > last_percent {
                    print!("\r    {}", progress.display());
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                    last_percent = pct;
                }
            }
        }
    }

    writer
        .flush()
        .await
        .with_context(|| format!("Failed to flush {}", dest.display()))?;

    if options.show_progress {
        println!();
    }

    // Verify downloaded size matches expected (if provided)
    if let Some(expected) = options.expected_size {
        let actual = std::fs::metadata(dest)
            .map(|m| m.len())
            .unwrap_or(0);
        if actual != expected {
            // Remove incomplete file
            let _ = std::fs::remove_file(dest);
            bail!(
                "Download incomplete for {}: expected {} bytes, got {} bytes",
                url,
                expected,
                actual
            );
        }
    }

    Ok(())
}

/// Check if an error is likely transient and worth retrying.
fn is_retryable_error(e: &anyhow::Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("timeout")
        || msg.contains("connection reset")
        || msg.contains("connection refused")
        || msg.contains("temporarily unavailable")
        || msg.contains("try again")
        || msg.contains("503")
        || msg.contains("502")
        || msg.contains("504")
}

// =============================================================================
// BitTorrent Downloads
// =============================================================================

/// Download a file via BitTorrent.
///
/// # Arguments
/// * `torrent_url` - URL to .torrent file
/// * `dest_dir` - Directory to download into (filename comes from torrent)
/// * `options` - Download options
///
/// # Returns
/// Path to the downloaded file (filename determined by torrent metadata)
///
/// # Errors
/// Returns detailed error with torrent URL and peer connection info.
pub async fn torrent(torrent_url: &str, dest_dir: &Path, options: &DownloadOptions) -> Result<PathBuf> {
    use librqbit::{AddTorrent, Session};

    if options.show_progress {
        println!("    Torrent: {}", torrent_url);
        println!("    Destination: {}", dest_dir.display());
    }

    // Use a unique temp directory to avoid picking up stale files
    let temp_dir = dest_dir.join(format!(".torrent-download-{}", std::process::id()));
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .with_context(|| format!("Failed to create temp directory {}", temp_dir.display()))?;

    // Cleanup helper - ensures temp dir is removed on failure
    let cleanup_temp = || {
        let _ = std::fs::remove_dir_all(&temp_dir);
    };

    // Create session in temp directory
    let session = match Session::new(temp_dir.clone()).await {
        Ok(s) => s,
        Err(e) => {
            cleanup_temp();
            return Err(e).with_context(|| format!("Failed to create BitTorrent session for {}", torrent_url));
        }
    };

    // Add torrent from URL
    let handle = match session.add_torrent(AddTorrent::from_url(torrent_url), None).await {
        Ok(added) => match added.into_handle() {
            Some(h) => h,
            None => {
                cleanup_temp();
                bail!("Failed to get torrent handle for {} - torrent may already exist", torrent_url);
            }
        },
        Err(e) => {
            cleanup_temp();
            return Err(e).with_context(|| format!("Failed to add torrent: {}", torrent_url));
        }
    };

    if options.show_progress {
        println!("    Downloading...");
    }

    // Poll for progress
    let mut last_percent = 0u64;
    let mut stall_count = 0u32;
    let mut last_progress = 0u64;

    loop {
        let stats = handle.stats();

        // Progress reporting
        let total = stats.total_bytes;
        if total > 0 && options.show_progress {
            let percent = stats.progress_bytes * 100 / total;
            if percent > last_percent {
                let progress = Progress::new(stats.progress_bytes, Some(total));
                print!("\r    {}", progress.display());
                use std::io::Write;
                std::io::stdout().flush().ok();
                last_percent = percent;
            }
        }

        // Check if done
        if stats.finished {
            if options.show_progress {
                println!();
            }
            break;
        }

        // Stall detection
        if stats.progress_bytes == last_progress {
            stall_count += 1;
            if stall_count > 300 {
                // 5 minutes with no progress
                cleanup_temp();
                bail!(
                    "BitTorrent download stalled for {} (no progress for 5 minutes)",
                    torrent_url
                );
            }
        } else {
            stall_count = 0;
            last_progress = stats.progress_bytes;
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // Find downloaded files in temp directory
    let entries: Vec<_> = std::fs::read_dir(&temp_dir)
        .with_context(|| format!("Failed to read directory {}", temp_dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            // Include files and directories, but skip hidden files (like .torrent metadata)
            !e.file_name().to_string_lossy().starts_with('.')
        })
        .collect();

    if entries.is_empty() {
        cleanup_temp();
        bail!("No files found in {} after torrent download", temp_dir.display());
    }

    // Handle single file vs directory
    let temp_item = entries[0].path();
    let (temp_file, filename) = if temp_item.is_file() {
        // Single file torrent
        let fname = temp_item.file_name()
            .with_context(|| format!("Downloaded file has no filename: {}", temp_item.display()))?
            .to_os_string();
        (temp_item.clone(), fname)
    } else if temp_item.is_dir() {
        // Directory torrent - look for the largest file inside
        let mut largest: Option<(PathBuf, u64)> = None;
        for entry in walkdir::WalkDir::new(&temp_item)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
        {
            if let Ok(meta) = entry.metadata() {
                let size = meta.len();
                if largest.is_none() || size > largest.as_ref().unwrap().1 {
                    largest = Some((entry.path().to_path_buf(), size));
                }
            }
        }
        match largest {
            Some((path, _)) => {
                let fname = path.file_name()
                    .with_context(|| format!("Downloaded file has no filename: {}", path.display()))?
                    .to_os_string();
                (path, fname)
            }
            None => {
                cleanup_temp();
                bail!("No files found in torrent directory {}", temp_item.display());
            }
        }
    } else {
        cleanup_temp();
        bail!("Unexpected item type in torrent download: {}", temp_item.display());
    };

    // Move to final destination
    let final_path = dest_dir.join(&filename);
    if final_path.exists() {
        std::fs::remove_file(&final_path)
            .with_context(|| format!("Failed to remove existing file {}", final_path.display()))?;
    }
    std::fs::rename(&temp_file, &final_path)
        .with_context(|| format!("Failed to move {} to {}", temp_file.display(), final_path.display()))?;

    // Cleanup temp directory (removes any other files from multi-file torrents)
    cleanup_temp();

    Ok(final_path)
}

// =============================================================================
// Git Clone
// =============================================================================

/// Clone a git repository.
///
/// # Arguments
/// * `url` - Git repository URL
/// * `dest` - Destination directory
/// * `shallow` - If true, use --depth 1 for faster clone
///
/// # Errors
/// Returns detailed error with git stderr output.
pub async fn git_clone(url: &str, dest: &Path, shallow: bool) -> Result<()> {
    git_clone_with_timeout(url, dest, shallow, Duration::from_secs(600)).await
}

/// Clone a git repository with configurable timeout.
///
/// # Arguments
/// * `url` - Git repository URL
/// * `dest` - Destination directory
/// * `shallow` - If true, use --depth 1 for faster clone
/// * `timeout` - Maximum time to wait for clone to complete
///
/// # Errors
/// Returns detailed error with git stderr output.
pub async fn git_clone_with_timeout(url: &str, dest: &Path, shallow: bool, timeout: Duration) -> Result<()> {
    use tokio::process::Command;

    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }

    // If destination exists, check if it's a valid git repo
    // A valid git repo has a .git directory (or is a bare repo with HEAD)
    if dest.exists() {
        let is_valid_git = dest.join(".git").exists() || dest.join("HEAD").exists();
        if !is_valid_git {
            // Directory exists but isn't a git repo - remove it
            // This handles broken/partial clones
            tokio::fs::remove_dir_all(dest)
                .await
                .with_context(|| format!("Failed to remove invalid directory {}", dest.display()))?;
        } else {
            // Valid git repo exists - git clone will fail, let caller handle
            // (they may want to pull instead)
            bail!(
                "Destination {} already exists and is a git repository. Remove it first or use git pull.",
                dest.display()
            );
        }
    }

    let mut cmd = Command::new("git");
    cmd.arg("clone");
    if shallow {
        cmd.args(["--depth", "1"]);
    }
    cmd.arg(url);
    cmd.arg(dest);

    let output = tokio::time::timeout(timeout, cmd.output())
        .await
        .with_context(|| format!("git clone timed out after {:?} for {}", timeout, url))?
        .with_context(|| format!("Failed to execute git clone for {}", url))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!(
            "git clone failed for {}\n  Exit code: {}\n  stderr: {}\n  stdout: {}",
            url,
            output.status.code().unwrap_or(-1),
            stderr.trim(),
            stdout.trim()
        );
    }

    Ok(())
}

// =============================================================================
// Disk Space Checking
// =============================================================================

/// Check if there's enough disk space for a download.
///
/// # Arguments
/// * `path` - Directory where download will be stored
/// * `required_bytes` - Minimum required free space
///
/// # Errors
/// Returns error if not enough space. Warns but continues if check fails.
pub fn check_disk_space(path: &Path, required_bytes: u64) -> Result<()> {
    // Try platform-specific methods to get available disk space
    let available = get_available_space(path);

    match available {
        Some(avail) => {
            if avail < required_bytes {
                let required_gb = required_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                let available_gb = avail as f64 / (1024.0 * 1024.0 * 1024.0);
                bail!(
                    "Not enough disk space in {}\n  Required: {:.1} GB\n  Available: {:.1} GB",
                    path.display(),
                    required_gb,
                    available_gb
                );
            }
            Ok(())
        }
        None => {
            // Could not determine disk space - warn loudly but continue
            let required_gb = required_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            eprintln!(
                "WARNING: Could not check disk space for {}. Ensure at least {:.1} GB is available.",
                path.display(),
                required_gb
            );
            Ok(())
        }
    }
}

/// Get available disk space in bytes. Returns None if check fails.
fn get_available_space(path: &Path) -> Option<u64> {
    use std::process::Command;

    // Try POSIX df first (works on Linux, macOS, BSD)
    // Use -k for kilobytes (universally supported) and parse the output
    let output = Command::new("df")
        .arg("-k") // Output in 1K blocks (POSIX standard)
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse df output - format is typically:
    // Filesystem     1K-blocks      Used Available Use% Mounted on
    // /dev/sda1      123456789  12345678  98765432  12% /
    // We want the 4th column (Available) from the second line
    let line = stdout.lines().nth(1)?;
    let fields: Vec<&str> = line.split_whitespace().collect();

    // Available is typically the 4th field (index 3)
    // But some systems have different formats, so try to find it
    if fields.len() >= 4 {
        // Try index 3 first (standard)
        if let Ok(kb) = fields[3].parse::<u64>() {
            return Some(kb * 1024); // Convert KB to bytes
        }
    }

    None
}

// =============================================================================
// Archive Extraction
// =============================================================================

/// Extract a tarball.
///
/// # Arguments
/// * `archive` - Path to .tar.gz or .tar.xz file
/// * `dest_dir` - Directory to extract into
/// * `strip_components` - Number of leading path components to strip (like tar --strip-components)
///
/// # Errors
/// Returns detailed error with tar stderr output.
#[allow(dead_code)]
pub async fn extract_tarball(
    archive: &Path,
    dest_dir: &Path,
    strip_components: u32,
) -> Result<()> {
    use tokio::process::Command;

    // Ensure destination exists
    tokio::fs::create_dir_all(dest_dir)
        .await
        .with_context(|| format!("Failed to create directory {}", dest_dir.display()))?;

    // Detect compression from extension
    let archive_str = archive.to_string_lossy();
    let tar_flag = if archive_str.ends_with(".tar.xz") || archive_str.ends_with(".txz") {
        "xJf"
    } else if archive_str.ends_with(".tar.gz") || archive_str.ends_with(".tgz") {
        "xzf"
    } else if archive_str.ends_with(".tar.bz2") || archive_str.ends_with(".tbz2") {
        "xjf"
    } else {
        "xf" // Plain .tar or let tar auto-detect
    };

    let mut cmd = Command::new("tar");
    cmd.arg(tar_flag);
    cmd.arg(archive);
    cmd.arg("-C");
    cmd.arg(dest_dir);
    if strip_components > 0 {
        cmd.arg(format!("--strip-components={}", strip_components));
    }

    let output = cmd
        .output()
        .await
        .with_context(|| format!("Failed to execute tar for {}", archive.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "tar extraction failed for {}\n  Exit code: {}\n  stderr: {}",
            archive.display(),
            output.status.code().unwrap_or(-1),
            stderr.trim()
        );
    }

    Ok(())
}

/// Extract a specific file from a tarball.
pub async fn extract_file_from_tarball(
    archive: &Path,
    dest_dir: &Path,
    filename: &str,
) -> Result<PathBuf> {
    use tokio::process::Command;

    // Ensure destination exists
    tokio::fs::create_dir_all(dest_dir)
        .await
        .with_context(|| format!("Failed to create directory {}", dest_dir.display()))?;

    // Detect compression from extension (same logic as extract_tarball)
    let archive_str = archive.to_string_lossy();
    let tar_flag = if archive_str.ends_with(".tar.xz") || archive_str.ends_with(".txz") {
        "xJf"
    } else if archive_str.ends_with(".tar.gz") || archive_str.ends_with(".tgz") {
        "xzf"
    } else if archive_str.ends_with(".tar.bz2") || archive_str.ends_with(".tbz2") {
        "xjf"
    } else {
        "xf" // Plain .tar or let tar auto-detect
    };

    let mut cmd = Command::new("tar");
    cmd.arg(tar_flag);
    cmd.arg(archive);
    cmd.args(["-C"]);
    cmd.arg(dest_dir);
    cmd.arg(filename);

    let output = cmd
        .output()
        .await
        .with_context(|| format!("Failed to execute tar for {}", archive.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "tar extraction failed for {} from {}\n  Exit code: {}\n  stderr: {}",
            filename,
            archive.display(),
            output.status.code().unwrap_or(-1),
            stderr.trim()
        );
    }

    Ok(dest_dir.join(filename))
}

// =============================================================================
// Checksum verification (re-exported from checksum module)
// =============================================================================

pub mod checksum {
    use anyhow::{bail, Context, Result};
    use sha2::{Digest, Sha256};
    use std::io::Read;
    use std::path::Path;

    /// Verify SHA256 checksum of a file.
    ///
    /// # Arguments
    /// * `path` - File to verify
    /// * `expected` - Expected SHA256 hash (lowercase hex)
    /// * `show_progress` - Whether to show progress for large files
    ///
    /// # Errors
    /// Returns detailed error with expected vs actual hash.
    pub fn verify_sha256(path: &Path, expected: &str, show_progress: bool) -> Result<()> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open {} for checksum", path.display()))?;

        let file_size = file
            .metadata()
            .with_context(|| format!("Failed to get metadata for {}", path.display()))?
            .len();

        let mut reader = std::io::BufReader::with_capacity(1024 * 1024, file);
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 1024 * 1024]; // 1MB chunks
        let mut total_read = 0u64;
        let mut last_percent = 0u8;

        loop {
            let bytes_read = reader
                .read(&mut buffer)
                .with_context(|| format!("Failed to read {}", path.display()))?;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
            total_read += bytes_read as u64;

            // Progress indicator for large files
            if show_progress && file_size > 100 * 1024 * 1024 {
                // Only show for files > 100MB
                let percent = ((total_read * 100) / file_size) as u8;
                if percent >= last_percent + 10 {
                    print!("    Checksum: {}%...", percent);
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                    last_percent = percent;
                }
            }
        }

        if show_progress && file_size > 100 * 1024 * 1024 {
            println!();
        }

        let result = hasher.finalize();
        let actual = format!("{:x}", result);

        if actual != expected.to_lowercase() {
            bail!(
                "Checksum mismatch for {}\n  Expected: {}\n  Actual:   {}",
                path.display(),
                expected,
                actual
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // =========================================================================
    // DownloadOptions tests
    // =========================================================================

    #[test]
    fn test_download_options_default() {
        let opts = DownloadOptions::default();
        assert_eq!(opts.retries, 3);
        assert!(opts.show_progress);
        assert!(opts.timeout.is_none()); // No timeout for large downloads
    }

    #[test]
    fn test_download_options_large_file() {
        let opts = DownloadOptions::large_file(1024 * 1024 * 100);
        assert_eq!(opts.expected_size, Some(100 * 1024 * 1024));
        assert!(opts.show_progress);
    }

    #[test]
    fn test_download_options_metadata() {
        let opts = DownloadOptions::metadata();
        assert!(!opts.show_progress);
        assert!(opts.timeout.is_some());
        assert_eq!(opts.timeout.unwrap().as_secs(), 30);
    }

    // =========================================================================
    // Progress display tests
    // =========================================================================

    #[test]
    fn test_progress_display_with_total() {
        let p = Progress::new(50 * 1024 * 1024, Some(100 * 1024 * 1024));
        let display = p.display();
        assert!(display.contains("50"), "Should show ~50 MB downloaded");
        assert!(display.contains("100"), "Should show ~100 MB total");
        assert!(display.contains("50%"), "Should show 50%");
    }

    #[test]
    fn test_progress_display_without_total() {
        let p = Progress::new(50 * 1024 * 1024, None);
        let display = p.display();
        assert!(display.contains("50"), "Should show ~50 MB");
        assert!(!display.contains("%"), "Should not show percentage without total");
    }

    #[test]
    fn test_progress_percent_calculation() {
        let p = Progress::new(25, Some(100));
        assert_eq!(p.percent, Some(25));

        let p2 = Progress::new(0, Some(100));
        assert_eq!(p2.percent, Some(0));

        let p3 = Progress::new(100, Some(100));
        assert_eq!(p3.percent, Some(100));
    }

    #[test]
    fn test_progress_zero_total() {
        let p = Progress::new(50, Some(0));
        assert_eq!(p.percent, Some(0)); // Avoid division by zero
    }

    // =========================================================================
    // Retry logic tests
    // =========================================================================

    #[test]
    fn test_is_retryable_timeout() {
        assert!(is_retryable_error(&anyhow::anyhow!("connection timeout")));
        assert!(is_retryable_error(&anyhow::anyhow!("request TIMEOUT")));
    }

    #[test]
    fn test_is_retryable_connection_errors() {
        assert!(is_retryable_error(&anyhow::anyhow!("connection reset by peer")));
        assert!(is_retryable_error(&anyhow::anyhow!("connection refused")));
        assert!(is_retryable_error(&anyhow::anyhow!("temporarily unavailable")));
    }

    #[test]
    fn test_is_retryable_server_errors() {
        assert!(is_retryable_error(&anyhow::anyhow!("HTTP 502 Bad Gateway")));
        assert!(is_retryable_error(&anyhow::anyhow!("HTTP 503 Service Unavailable")));
        assert!(is_retryable_error(&anyhow::anyhow!("HTTP 504 Gateway Timeout")));
    }

    #[test]
    fn test_is_not_retryable() {
        assert!(!is_retryable_error(&anyhow::anyhow!("HTTP 404 Not Found")));
        assert!(!is_retryable_error(&anyhow::anyhow!("HTTP 401 Unauthorized")));
        assert!(!is_retryable_error(&anyhow::anyhow!("file not found")));
        assert!(!is_retryable_error(&anyhow::anyhow!("invalid checksum")));
    }

    // =========================================================================
    // Checksum tests
    // =========================================================================

    #[test]
    fn test_verify_sha256_valid() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"hello world").unwrap();
        file.flush().unwrap();

        // SHA256 of "hello world"
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";

        let result = checksum::verify_sha256(file.path(), expected, false);
        assert!(result.is_ok(), "Valid checksum should pass: {:?}", result);
    }

    #[test]
    fn test_verify_sha256_invalid() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"hello world").unwrap();
        file.flush().unwrap();

        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let result = checksum::verify_sha256(file.path(), wrong_hash, false);
        assert!(result.is_err(), "Invalid checksum should fail");

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Checksum mismatch"), "Error should mention mismatch");
        assert!(err_msg.contains("Expected"), "Error should show expected hash");
        assert!(err_msg.contains("Actual"), "Error should show actual hash");
    }

    #[test]
    fn test_verify_sha256_empty_file() {
        let file = NamedTempFile::new().unwrap();

        // SHA256 of empty file
        let expected = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

        let result = checksum::verify_sha256(file.path(), expected, false);
        assert!(result.is_ok(), "Empty file checksum should work");
    }

    #[test]
    fn test_verify_sha256_case_insensitive() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"test").unwrap();
        file.flush().unwrap();

        // SHA256 of "test" - uppercase
        let expected = "9F86D081884C7D659A2FEAA0C55AD015A3BF4F1B2B0B822CD15D6C15B0F00A08";

        let result = checksum::verify_sha256(file.path(), expected, false);
        assert!(result.is_ok(), "Uppercase hash should work");
    }

    #[test]
    fn test_verify_sha256_missing_file() {
        let result = checksum::verify_sha256(
            Path::new("/nonexistent/file.iso"),
            "abc123",
            false,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to open"));
    }

    // =========================================================================
    // Integration-style tests (still fast, no network)
    // =========================================================================

    #[test]
    fn test_download_options_retry_delay_exponential() {
        let opts = DownloadOptions::default();
        let base = opts.retry_delay;

        // Verify we can calculate exponential backoff
        let delay_1 = base * (1 << 0); // 2 seconds
        let delay_2 = base * (1 << 1); // 4 seconds
        let delay_3 = base * (1 << 2); // 8 seconds

        assert!(delay_2 > delay_1);
        assert!(delay_3 > delay_2);
    }
}
