//! The first-run download of `ggml-<size>.bin`, checked against pinned sizes and SHA-256 values.

use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::engine::{EngineError, ModelSize, normalize_base_url};

/// The LFS object of each file in `ggerganov/whisper.cpp` on Hugging Face (`main`, commit
/// `5359861c739e955e79d9a303bcbc70fb988958b1`): `GET /api/models/ggerganov/whisper.cpp/tree/main` → `lfs`.
struct Pinned {
    bytes: u64,
    sha256: &'static str,
}

const fn pinned(size: ModelSize) -> Pinned {
    match size {
        ModelSize::Tiny => Pinned {
            bytes: 77_691_713,
            sha256: "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
        },
        ModelSize::Base => Pinned {
            bytes: 147_951_465,
            sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
        },
        ModelSize::Small => Pinned {
            bytes: 487_601_967,
            sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
        },
    }
}

/// Where the whisper.cpp models live and where a missing one downloads from. The default directory is
/// `<cache dir>/happybirthdayvalera/models`; the default host is `behavior.json` → `models.desktop.baseUrl`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelStore {
    dir: PathBuf,
    base_url: Option<String>,
}

impl Default for ModelStore {
    fn default() -> Self {
        let base = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
        Self { dir: base.join("happybirthdayvalera").join("models"), base_url: None }
    }
}

impl ModelStore {
    /// A store in `dir`.
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into(), base_url: None }
    }

    /// Downloads `ggml-<size>.bin` from `<base_url>ggml-<size>.bin` instead of the Hugging Face Hub. A missing
    /// trailing `/` is added. The file is still checked against the pinned size and SHA-256.
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl AsRef<str>) -> Self {
        self.base_url = Some(normalize_base_url(base_url.as_ref()));
        self
    }

    /// The download URL of the model file.
    #[must_use]
    pub fn url(&self, size: ModelSize) -> String {
        match &self.base_url {
            Some(base) => format!("{base}{}", size.ggml_file_name()),
            None => size.ggml_url(),
        }
    }

    /// The directory of the store.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The path of the model file, whether or not it exists yet.
    #[must_use]
    pub fn path(&self, size: ModelSize) -> PathBuf {
        self.dir.join(size.ggml_file_name())
    }

    /// Returns the model path, downloading the file first when it is missing.
    ///
    /// The download goes to `<file>.part`, is checked against the pinned size and SHA-256, and is renamed only
    /// after the check, so a file with the final name was verified once. `progress` gets fractions in 0..=1.
    ///
    /// # Errors
    ///
    /// When the download, the check or a file operation fails.
    pub fn ensure(&self, size: ModelSize, progress: &mut dyn FnMut(f32)) -> Result<PathBuf, EngineError> {
        let pin = pinned(size);
        let path = self.path(size);
        match fs::metadata(&path) {
            Ok(meta) if meta.len() == pin.bytes => {
                progress(1.0);
                return Ok(path);
            }
            Ok(meta) => tracing::warn!(
                "{} has {} bytes instead of {}; downloading it again",
                path.display(),
                meta.len(),
                pin.bytes
            ),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_error("read", &path, &e)),
        }
        fs::create_dir_all(&self.dir).map_err(|e| io_error("create", &self.dir, &e))?;

        let url = self.url(size);
        let part = path.with_file_name(format!("{}.part", size.ggml_file_name()));
        tracing::info!("downloading {url} to {}", part.display());
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
            .build()
            .new_agent();
        let response = agent.get(&url).call().map_err(|e| EngineError::new(format!("GET {url}: {e}")))?;
        // A limit one byte above the pinned size turns an oversized body into an error instead of a full disk.
        let body = response.into_body().into_with_config().limit(pin.bytes + 1).reader();

        let file = File::create(&part).map_err(|e| io_error("create", &part, &e))?;
        let copied = copy_verified(body, BufWriter::new(file), &pin, progress);
        if let Err(e) = copied {
            if let Err(rm) = fs::remove_file(&part) {
                tracing::warn!("cannot remove {}: {rm}", part.display());
            }
            return Err(EngineError::new(format!("{url}: {e}")));
        }
        fs::rename(&part, &path).map_err(|e| io_error("rename into", &path, &e))?;
        Ok(path)
    }
}

fn io_error(action: &str, path: &Path, e: &io::Error) -> EngineError {
    EngineError::new(format!("cannot {action} {}: {e}", path.display()))
}

/// Copies `reader` to `writer` while hashing, then checks the length and the SHA-256 against `pin`.
fn copy_verified(
    mut reader: impl Read,
    mut writer: impl Write,
    pin: &Pinned,
    progress: &mut dyn FnMut(f32),
) -> Result<(), String> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 256 * 1024];
    let mut done: u64 = 0;
    let mut reported = -1.0_f32;
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(format!("read failed after {done} bytes: {e}")),
        };
        hasher.update(&buf[..n]);
        writer.write_all(&buf[..n]).map_err(|e| format!("write failed: {e}"))?;
        done += n as u64;
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, reason = "a progress fraction")]
        let fraction = ((done as f64 / pin.bytes as f64).min(1.0)) as f32;
        // Report in steps of half a percent: a 148 MB download would otherwise send ~600 updates to the UI.
        if fraction - reported >= 0.005 {
            reported = fraction;
            progress(fraction);
        }
    }
    writer.flush().map_err(|e| format!("write failed: {e}"))?;
    if done != pin.bytes {
        return Err(format!("got {done} bytes, expected {}", pin.bytes));
    }
    let actual = hex(&hasher.finalize());
    if actual != pin.sha256 {
        return Err(format!("SHA-256 mismatch: got {actual}, expected {}", pin.sha256));
    }
    progress(1.0);
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
        // Writing to a String cannot fail.
        let _ = write!(s, "{b:02x}");
        s
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // SHA-256 of the three bytes "abc" (FIPS 180-2, appendix B.1).
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn verified_copy_accepts_the_right_bytes() {
        let mut out = Vec::new();
        let mut seen = Vec::new();
        copy_verified(&b"abc"[..], &mut out, &Pinned { bytes: 3, sha256: ABC }, &mut |f| seen.push(f)).unwrap();
        assert_eq!(out, b"abc");
        assert_eq!(seen.last().copied(), Some(1.0));
    }

    #[test]
    fn verified_copy_rejects_a_wrong_hash_or_length() {
        const WRONG: &str = "0000000000000000000000000000000000000000000000000000000000000000";
        let err = copy_verified(&b"abc"[..], io::sink(), &Pinned { bytes: 3, sha256: WRONG }, &mut |_| {});
        assert!(err.unwrap_err().contains("SHA-256 mismatch"));
        let err = copy_verified(&b"ab"[..], io::sink(), &Pinned { bytes: 3, sha256: ABC }, &mut |_| {});
        assert!(err.unwrap_err().contains("expected 3"));
    }

    #[test]
    fn every_size_is_pinned() {
        for size in ModelSize::ALL {
            let pin = pinned(size);
            assert_eq!(pin.sha256.len(), 64, "{size}");
            assert!(pin.sha256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
            assert!(pin.bytes > 10_000_000);
        }
    }

    #[test]
    fn a_base_url_replaces_the_hub_for_the_download_only() {
        let hub = ModelStore::at("/tmp/hbv");
        assert_eq!(hub.url(ModelSize::Tiny), ModelSize::Tiny.ggml_url());
        let hosted = ModelStore::at("/tmp/hbv").with_base_url("http://localhost:5299");
        assert_eq!(hosted.url(ModelSize::Small), "http://localhost:5299/ggml-small.bin");
        assert_eq!(hosted.path(ModelSize::Small), hub.path(ModelSize::Small));
    }

    #[test]
    fn an_existing_file_of_the_pinned_size_is_used_without_network() {
        let dir = std::env::temp_dir().join(format!("hbv-store-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let store = ModelStore::at(&dir);
        let path = store.path(ModelSize::Tiny);
        let file = File::create(&path).unwrap();
        file.set_len(pinned(ModelSize::Tiny).bytes).unwrap();
        let mut last = 0.0;
        assert_eq!(store.ensure(ModelSize::Tiny, &mut |f| last = f).unwrap(), path);
        assert!((last - 1.0).abs() < f32::EPSILON);
        fs::remove_dir_all(&dir).unwrap();
    }
}
