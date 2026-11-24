use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};

use docsuite_core::{Error, Result};
use sha2::Digest;

pub const DETECTION_FILE: &str = "text-detection.onnx";
pub const RECOGNITION_FILE: &str = "text-recognition.onnx";

pub const DETECTION_SHA256: &str =
    "a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe";
pub const RECOGNITION_SHA256: &str =
    "86c145c2edb96c8caed5b1ebb8f44d706408922211451c309c625157dd6061c5";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelStatus {
    pub detection: bool,
    pub recognition: bool,
}

impl ModelStatus {
    pub fn ready(&self) -> bool {
        self.detection && self.recognition
    }
}

pub fn default_dir() -> PathBuf {
    if let Ok(dir) = env::var("DOCAPP_MODELS") {
        return PathBuf::from(dir);
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home).join(".local/share/docapp/models");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home).join("Library/Application Support/docapp/models");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("docapp/models");
        }
    }
    PathBuf::from("models")
}

pub fn model_paths(dir: &Path) -> (PathBuf, PathBuf) {
    (dir.join(DETECTION_FILE), dir.join(RECOGNITION_FILE))
}

pub fn status(dir: &Path) -> ModelStatus {
    let (det, rec) = model_paths(dir);
    ModelStatus {
        detection: det.is_file(),
        recognition: rec.is_file(),
    }
}

pub fn verify_file(path: &Path, expected_hex: &str) -> Result<()> {
    let mut file = std::fs::File::open(path).map_err(Error::from)?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).map_err(Error::from)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if hex != expected_hex {
        return Err(Error::Ocr(format!(
            "sha256 mismatch for {} (expected {}, got {})",
            path.display(),
            expected_hex,
            hex
        )));
    }
    Ok(())
}

pub fn ensure_models(dir: &Path) -> Result<()> {
    let st = status(dir);
    if !st.ready() {
        return Err(Error::Ocr(
            "ocr models missing; run `docapp models fetch`".to_owned(),
        ));
    }
    let (det, rec) = model_paths(dir);
    verify_file(&det, DETECTION_SHA256)?;
    verify_file(&rec, RECOGNITION_SHA256)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dir_non_empty() {
        let dir = default_dir();
        assert!(!dir.as_os_str().is_empty());
    }

    #[test]
    fn verify_rejects_wrong_hash() {
        let dir = std::env::temp_dir().join(format!("docapp-sha-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("x.onnx");
        std::fs::write(&path, b"bad").unwrap();
        assert!(verify_file(&path, DETECTION_SHA256).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
