//! File-system API of the `.dti.bin` binary cache (host only: `std`).
//!
//! The cache lives next to its source: a final `.awdti` extension is replaced
//! by `.dti.bin` (`foo.awdti` → `foo.dti.bin`); any other name gets `.dti.bin`
//! appended. Any problem reading the cache is a cache miss; after a recompile the
//! cache is rewritten through a temporary sibling and a rename, so readers never
//! observe a partially written binary.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use apriwave_core::dti::{
    load_or_compile_local, load_or_compile_universal, DtiDiagnostic, DtiErrorKind, DtiLoadError,
    DtiSource, LocalOverlay, UniversalSnapshot, MAX_BINARY_BYTES, MAX_SOURCE_BYTES,
};

/// Suffix of the binary cache file (see [`binary_cache_path`]).
pub const BINARY_SUFFIX: &str = ".dti.bin";
/// Extension of DTI text sources.
pub const SOURCE_EXTENSION: &str = "awdti";

/// Failure of the file-system API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DtiFileError {
    /// File-system error.
    Io(io::ErrorKind),
    /// The source (or the binary) failed to load.
    Load(DtiLoadError),
}

impl fmt::Display for DtiFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(kind) => write!(f, "I/O error: {kind}"),
            Self::Load(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for DtiFileError {}

impl From<DtiLoadError> for DtiFileError {
    fn from(err: DtiLoadError) -> Self {
        Self::Load(err)
    }
}

/// Path of the binary cache of `source_path`, in the same directory: a final
/// `.awdti` extension is replaced by `.dti.bin` (`foo.awdti` → `foo.dti.bin`);
/// any other name gets `.dti.bin` appended (`foo.txt` → `foo.txt.dti.bin`).
pub fn binary_cache_path(source_path: &Path) -> PathBuf {
    let base = if source_path
        .extension()
        .is_some_and(|ext| ext == SOURCE_EXTENSION)
    {
        source_path.with_extension("")
    } else {
        source_path.to_path_buf()
    };
    let mut name = base.into_os_string();
    name.push(BINARY_SUFFIX);
    PathBuf::from(name)
}

fn read_source(path: &Path, source: DtiSource) -> Result<String, DtiFileError> {
    let size = fs::metadata(path)
        .map_err(|e| DtiFileError::Io(e.kind()))?
        .len();
    if size > u64::try_from(MAX_SOURCE_BYTES).unwrap_or(u64::MAX) {
        return Err(DtiFileError::Load(DtiLoadError::Source(vec![
            DtiDiagnostic {
                source,
                line: 0,
                kind: DtiErrorKind::SourceTooLarge,
            },
        ])));
    }
    fs::read_to_string(path).map_err(|e| DtiFileError::Io(e.kind()))
}

/// Reads the cache; any problem (missing, unreadable, oversized) is a cache miss.
fn read_cache(path: &Path) -> Option<Vec<u8>> {
    let metadata = fs::metadata(path).ok()?;
    let limit = u64::try_from(MAX_BINARY_BYTES).unwrap_or(u64::MAX);
    if !metadata.is_file() || metadata.len() > limit {
        return None;
    }
    fs::read(path).ok()
}

/// Writes the cache through a temporary sibling and a rename, so readers never
/// observe a partially written binary.
fn write_cache(path: &Path, bytes: &[u8]) -> Result<(), DtiFileError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(format!(".tmp{}-{unique}", std::process::id()));
    let temporary = PathBuf::from(temporary);
    fs::write(&temporary, bytes).map_err(|e| DtiFileError::Io(e.kind()))?;
    fs::rename(&temporary, path).map_err(|e| {
        // Best effort: the rename error is the one worth reporting.
        let _ = fs::remove_file(&temporary);
        DtiFileError::Io(e.kind())
    })
}

/// Loads a universal snapshot from `source_path`, using the sibling binary
/// cache [`binary_cache_path`] when it is valid and up to date, and rewriting
/// it otherwise.
pub fn load_or_compile_universal_path(
    source_path: &Path,
) -> Result<UniversalSnapshot, DtiFileError> {
    let source = read_source(source_path, DtiSource::Universal)?;
    let cache = binary_cache_path(source_path);
    let binary = read_cache(&cache);
    let (snapshot, fresh) = load_or_compile_universal(&source, binary.as_deref())?;
    if let Some(bytes) = fresh {
        write_cache(&cache, &bytes)?;
    }
    Ok(snapshot)
}

/// Local counterpart of [`load_or_compile_universal_path`].
pub fn load_or_compile_local_path(
    source_path: &Path,
    universal: &UniversalSnapshot,
) -> Result<LocalOverlay, DtiFileError> {
    let source = read_source(source_path, DtiSource::Local)?;
    let cache = binary_cache_path(source_path);
    let binary = read_cache(&cache);
    let (overlay, fresh) = load_or_compile_local(&source, binary.as_deref(), universal)?;
    if let Some(bytes) = fresh {
        write_cache(&cache, &bytes)?;
    }
    Ok(overlay)
}

#[cfg(test)]
#[path = "dti_fs_tests.rs"]
mod tests;
