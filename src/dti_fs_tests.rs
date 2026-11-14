//! File-system API of the `.dti.bin` cache: naming, round trip, stale and
//! corrupt caches, missing sources.

use std::path::{Path, PathBuf};

use super::{
    binary_cache_path, load_or_compile_local_path, load_or_compile_universal_path, DtiFileError,
};
use apriwave_core::dti::{fnv1a_64, DtiKey};

type TestResult = Result<(), String>;

fn k(tag: &str) -> Result<String, String> {
    let mut body = [b'0'; 31];
    for (slot, byte) in body.iter_mut().zip(tag.bytes()) {
        *slot = byte;
    }
    DtiKey::from_random_body(body)
        .map(|key| key.as_str().to_owned())
        .map_err(|e| e.to_string())
}

fn check<T: PartialEq + std::fmt::Debug>(what: &str, got: T, want: T) -> TestResult {
    if got == want {
        Ok(())
    } else {
        Err(format!("{what}: got {got:?}, want {want:?}"))
    }
}

fn universal_source(version: u64) -> Result<String, String> {
    Ok(format!(
        "#awdti 1\n#role universal\n#version {version}\n#lang en\nrec {green} green syn:verdant\nrec {door} door\n",
        green = k("green")?,
        door = k("door")?,
    ))
}

fn local_source(universal_version: u64) -> Result<String, String> {
    Ok(format!(
        "#awdti 1\n#role local\n#universal {universal_version}\n#lang it\nuse {green} form:verde\nuse {door} form:porta\n",
        green = k("green")?,
        door = k("door")?,
    ))
}

#[test]
fn binary_cache_path_naming() -> TestResult {
    check(
        "awdti",
        binary_cache_path(Path::new("dir/foo.awdti")),
        PathBuf::from("dir/foo.dti.bin"),
    )?;
    check(
        "other extension",
        binary_cache_path(Path::new("dir/foo.txt")),
        PathBuf::from("dir/foo.txt.dti.bin"),
    )?;
    check(
        "no extension",
        binary_cache_path(Path::new("foo")),
        PathBuf::from("foo.dti.bin"),
    )
}

fn unique_temp_dir() -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("apriwave-dti-bin-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn path_round_trip(dir: &Path) -> TestResult {
    let universal_path = dir.join("en.awdti");
    let local_path = dir.join("it.awdti");
    let universal_src = universal_source(42)?;
    let local_src = local_source(42)?;
    std::fs::write(&universal_path, &universal_src).map_err(|e| e.to_string())?;
    std::fs::write(&local_path, &local_src).map_err(|e| e.to_string())?;
    let universal_bin = dir.join("en.dti.bin");
    let local_bin = dir.join("it.dti.bin");

    // First load compiles and writes the caches.
    let snapshot = load_or_compile_universal_path(&universal_path).map_err(|e| e.to_string())?;
    let overlay = load_or_compile_local_path(&local_path, &snapshot).map_err(|e| e.to_string())?;
    let written = std::fs::read(&universal_bin).map_err(|e| e.to_string())?;
    check(
        "universal cache",
        &written,
        &snapshot.to_binary(fnv1a_64(universal_src.as_bytes())),
    )?;
    let written_local = std::fs::read(&local_bin).map_err(|e| e.to_string())?;
    check(
        "local cache",
        &written_local,
        &overlay.to_binary(fnv1a_64(local_src.as_bytes())),
    )?;

    // Second load hits the caches.
    let again = load_or_compile_universal_path(&universal_path).map_err(|e| e.to_string())?;
    check("cached universal", &again, &snapshot)?;
    let again = load_or_compile_local_path(&local_path, &snapshot).map_err(|e| e.to_string())?;
    check("cached local", &again, &overlay)?;

    // A corrupt cache is rebuilt.
    std::fs::write(&universal_bin, b"garbage").map_err(|e| e.to_string())?;
    let rebuilt = load_or_compile_universal_path(&universal_path).map_err(|e| e.to_string())?;
    check("rebuilt model", &rebuilt, &snapshot)?;
    check(
        "rebuilt cache",
        std::fs::read(&universal_bin).map_err(|e| e.to_string())?,
        written,
    )?;

    // An edited source invalidates the cache.
    let edited = universal_src.replace("#version 42", "#version 43");
    std::fs::write(&universal_path, &edited).map_err(|e| e.to_string())?;
    let updated = load_or_compile_universal_path(&universal_path).map_err(|e| e.to_string())?;
    check("updated version", updated.version(), 43)?;
    check(
        "updated cache",
        std::fs::read(&universal_bin).map_err(|e| e.to_string())?,
        updated.to_binary(fnv1a_64(edited.as_bytes())),
    )?;

    // Missing sources are I/O errors.
    check(
        "missing source",
        load_or_compile_universal_path(&dir.join("missing.awdti")).map(|_| ()),
        Err(DtiFileError::Io(std::io::ErrorKind::NotFound)),
    )
}

#[test]
fn path_api_round_trip() -> TestResult {
    let dir = unique_temp_dir()?;
    let result = path_round_trip(&dir);
    let cleanup = std::fs::remove_dir_all(&dir).map_err(|e| e.to_string());
    result.and(cleanup)
}
