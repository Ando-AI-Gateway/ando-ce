//! File-based audit log writer with automatic daily rotation.
//!
//! When `AuditLogConfig.file_path` is set, audit records are written to the
//! specified file. The writer rotates the file daily (at UTC midnight) by
//! renaming the current file with a date suffix (e.g. `audit.log.2025-01-15`)
//! and creating a new file.
//!
//! Also supports rotation by file size (`max_file_size_bytes`).
//!
//! Thread-safe: uses a `Mutex<BufWriter>` internally so multiple proxy
//! workers can write concurrently (though in practice writes happen from the
//! single admin/audit thread).

use chrono::{NaiveDate, Utc};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{debug, error, info, warn};

// ── Configuration ────────────────────────────────────────────────────────────

/// Extended audit log rotation settings.
///
/// This lives outside the core config crate to avoid adding rotation-specific
/// fields to the compliance config. Users configure it via `compliance.audit_log`
/// fields.
#[derive(Debug, Clone)]
pub struct AuditFileConfig {
    /// Base file path, e.g. `/var/log/ando/audit.log`.
    pub file_path: PathBuf,
    /// Maximum file size in bytes before forced rotation.
    /// 0 = size-based rotation disabled (daily rotation only).
    pub max_file_size_bytes: u64,
    /// Maximum number of rotated files to keep. 0 = unlimited.
    pub max_rotated_files: usize,
}

impl Default for AuditFileConfig {
    fn default() -> Self {
        Self {
            file_path: PathBuf::from("audit.log"),
            max_file_size_bytes: 100 * 1024 * 1024, // 100 MB
            max_rotated_files: 30,
        }
    }
}

// ── Writer ───────────────────────────────────────────────────────────────────

/// A rotating audit log file writer.
///
/// Call [`AuditFileWriter::write_line`] to append a JSON audit line.
/// Rotation happens automatically.
pub struct AuditFileWriter {
    config: AuditFileConfig,
    inner: Mutex<WriterState>,
}

struct WriterState {
    writer: BufWriter<File>,
    current_date: NaiveDate,
    current_size: u64,
}

impl AuditFileWriter {
    /// Create a new writer, opening (or creating) the audit log file.
    pub fn new(config: AuditFileConfig) -> io::Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = config.file_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.file_path)?;

        let current_size = file.metadata()?.len();
        let today = Utc::now().date_naive();

        info!(path = %config.file_path.display(), "Audit log file writer opened");

        Ok(Self {
            config,
            inner: Mutex::new(WriterState {
                writer: BufWriter::new(file),
                current_date: today,
                current_size,
            }),
        })
    }

    /// Write a single JSON audit line. Rotates if needed.
    pub fn write_line(&self, line: &str) -> io::Result<()> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "audit writer lock poisoned"))?;

        let today = Utc::now().date_naive();

        // Check rotation conditions
        let needs_date_rotate = today != state.current_date;
        let needs_size_rotate = self.config.max_file_size_bytes > 0
            && state.current_size >= self.config.max_file_size_bytes;

        if needs_date_rotate || needs_size_rotate {
            // Flush current writer before rotation
            state.writer.flush()?;
            drop(std::mem::replace(
                &mut state.writer,
                BufWriter::new(File::create("/dev/null")?),
            ));

            // Generate rotated file name
            let suffix = if needs_date_rotate {
                state.current_date.format("%Y-%m-%d").to_string()
            } else {
                Utc::now().format("%Y-%m-%d-%H%M%S").to_string()
            };

            let rotated_path = rotated_file_path(&self.config.file_path, &suffix);

            // Rename current file
            if self.config.file_path.exists() {
                if let Err(e) = fs::rename(&self.config.file_path, &rotated_path) {
                    error!(
                        error = %e,
                        from = %self.config.file_path.display(),
                        to = %rotated_path.display(),
                        "Failed to rotate audit log"
                    );
                } else {
                    info!(
                        from = %self.config.file_path.display(),
                        to = %rotated_path.display(),
                        "Rotated audit log"
                    );
                }
            }

            // Prune old rotated files
            if self.config.max_rotated_files > 0 {
                if let Err(e) =
                    prune_rotated_files(&self.config.file_path, self.config.max_rotated_files)
                {
                    warn!(error = %e, "Failed to prune old audit log files");
                }
            }

            // Open new file
            let new_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.config.file_path)?;
            state.writer = BufWriter::new(new_file);
            state.current_date = today;
            state.current_size = 0;
        }

        // Write the line
        let bytes = line.as_bytes();
        state.writer.write_all(bytes)?;
        state.writer.write_all(b"\n")?;
        state.writer.flush()?;
        state.current_size += bytes.len() as u64 + 1;

        Ok(())
    }

    /// Flush buffered data to disk.
    pub fn flush(&self) -> io::Result<()> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "audit writer lock poisoned"))?;
        state.writer.flush()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Generate the rotated file path: `audit.log` → `audit.log.2025-01-15`.
fn rotated_file_path(base: &Path, suffix: &str) -> PathBuf {
    let mut path = base.as_os_str().to_owned();
    path.push(".");
    path.push(suffix);
    PathBuf::from(path)
}

/// Remove old rotated files, keeping only the newest `keep` files.
fn prune_rotated_files(base_path: &Path, keep: usize) -> io::Result<()> {
    let parent = base_path.parent().unwrap_or(Path::new("."));
    let base_name = base_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let mut rotated_files: Vec<PathBuf> = Vec::new();

    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&*base_name)
            && name.len() > base_name.len()
            && name.as_bytes()[base_name.len()] == b'.'
        {
            rotated_files.push(entry.path());
        }
    }

    // Sort by name (dates sort lexicographically) — newest last
    rotated_files.sort();

    if rotated_files.len() > keep {
        let to_remove = rotated_files.len() - keep;
        for path in rotated_files.iter().take(to_remove) {
            debug!(path = %path.display(), "Pruning old rotated audit log");
            fs::remove_file(path)?;
        }
    }

    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    use std::sync::atomic::{AtomicU64, Ordering as AtomOrd};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, AtomOrd::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "ando-audit-test-{}-{}",
            std::process::id(),
            n,
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rotated_file_path_appends_date_suffix() {
        let p = rotated_file_path(Path::new("/var/log/audit.log"), "2025-01-15");
        assert_eq!(p, PathBuf::from("/var/log/audit.log.2025-01-15"));
    }

    #[test]
    fn rotated_file_path_handles_no_extension() {
        let p = rotated_file_path(Path::new("audit"), "2025-01-15");
        assert_eq!(p, PathBuf::from("audit.2025-01-15"));
    }

    #[test]
    fn writer_creates_file_and_writes_line() {
        let dir = temp_dir();
        let path = dir.join("audit.log");
        let config = AuditFileConfig {
            file_path: path.clone(),
            max_file_size_bytes: 0,
            max_rotated_files: 0,
        };
        let writer = AuditFileWriter::new(config).unwrap();
        writer.write_line(r#"{"event":"test"}"#).unwrap();

        let mut content = String::new();
        File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert!(content.contains(r#"{"event":"test"}"#));
        assert!(content.ends_with('\n'));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn writer_appends_multiple_lines() {
        let dir = temp_dir();
        let path = dir.join("audit.log");
        let config = AuditFileConfig {
            file_path: path.clone(),
            max_file_size_bytes: 0,
            max_rotated_files: 0,
        };
        let writer = AuditFileWriter::new(config).unwrap();
        writer.write_line("line1").unwrap();
        writer.write_line("line2").unwrap();
        writer.write_line("line3").unwrap();

        let mut content = String::new();
        File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines, vec!["line1", "line2", "line3"]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn size_based_rotation_creates_rotated_file() {
        let dir = temp_dir();
        let path = dir.join("audit.log");
        let config = AuditFileConfig {
            file_path: path.clone(),
            max_file_size_bytes: 10, // Very small — triggers rotation quickly
            max_rotated_files: 5,
        };
        let writer = AuditFileWriter::new(config).unwrap();

        // Write enough to exceed 10 bytes
        writer.write_line("abcdefghijklmnop").unwrap(); // 17 bytes > 10
        // Next write should trigger rotation
        writer.write_line("second-line").unwrap();

        // Current file should exist with only the second line
        let mut content = String::new();
        File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert!(content.contains("second-line"));

        // There should be a rotated file
        let entries: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(entries.len() >= 2, "Expected rotated file, got {:?}", entries);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_keeps_only_specified_count() {
        let dir = temp_dir();
        let base = dir.join("audit.log");

        // Create dummy rotated files
        for i in 1..=5 {
            let p = dir.join(format!("audit.log.2025-01-{:02}", i));
            File::create(&p).unwrap();
        }

        prune_rotated_files(&base, 2).unwrap();

        let remaining: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("audit.log.")
            })
            .collect();
        assert_eq!(remaining.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_config_values() {
        let config = AuditFileConfig::default();
        assert_eq!(config.max_file_size_bytes, 100 * 1024 * 1024);
        assert_eq!(config.max_rotated_files, 30);
    }

    #[test]
    fn writer_creates_parent_directories() {
        let dir = temp_dir();
        let path = dir.join("deep").join("nested").join("audit.log");
        let config = AuditFileConfig {
            file_path: path.clone(),
            max_file_size_bytes: 0,
            max_rotated_files: 0,
        };
        let writer = AuditFileWriter::new(config).unwrap();
        writer.write_line("nested-test").unwrap();
        assert!(path.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn flush_does_not_error_on_fresh_writer() {
        let dir = temp_dir();
        let path = dir.join("audit.log");
        let config = AuditFileConfig {
            file_path: path,
            max_file_size_bytes: 0,
            max_rotated_files: 0,
        };
        let writer = AuditFileWriter::new(config).unwrap();
        writer.flush().unwrap();

        let _ = fs::remove_dir_all(&dir);
    }
}
