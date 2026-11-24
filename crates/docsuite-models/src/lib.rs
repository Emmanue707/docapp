use std::path::PathBuf;

#[cfg(not(feature = "download"))]
use docsuite_core::{Error, Result};
pub use docsuite_ocr::{
    DETECTION_FILE, DETECTION_SHA256, HANDWRITING_FILE, HANDWRITING_SHA256, ModelStatus,
    RECOGNITION_FILE, RECOGNITION_SHA256, default_dir, handwriting_path, model_paths, status,
    verify_file,
};

pub const DETECTION_MODEL_URL: &str =
    "https://github.com/Emmanue707/docapp/releases/download/models-v1/text-detection.onnx";
pub const RECOGNITION_MODEL_URL: &str =
    "https://github.com/Emmanue707/docapp/releases/download/models-v1/text-recognition.onnx";
pub const HANDWRITING_MODEL_URL: &str =
    "https://github.com/Emmanue707/docapp/releases/download/models-v1/handwriting-recognition.onnx";

pub const DETECTION_MODEL_URL_FALLBACK: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx";
pub const RECOGNITION_MODEL_URL_FALLBACK: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx";
pub const HANDWRITING_MODEL_URL_FALLBACK: &str =
    "https://huggingface.co/PellelNitram/xournalpp-htr-simple-htr/resolve/main/model.onnx";

pub fn models_dir(custom: Option<PathBuf>) -> PathBuf {
    custom.unwrap_or_else(default_dir)
}

#[cfg(feature = "download")]
mod fetch {
    use std::fs;
    use std::io::{Read, Write};
    use std::path::Path;

    use anyhow::{Context, Result, bail};
    use sha2::Digest;

    use super::{
        DETECTION_FILE, DETECTION_MODEL_URL, DETECTION_MODEL_URL_FALLBACK, DETECTION_SHA256,
        HANDWRITING_FILE, HANDWRITING_MODEL_URL, HANDWRITING_MODEL_URL_FALLBACK,
        HANDWRITING_SHA256, RECOGNITION_FILE, RECOGNITION_MODEL_URL,
        RECOGNITION_MODEL_URL_FALLBACK, RECOGNITION_SHA256, verify_file,
    };

    pub fn fetch_models(dir: &Path) -> Result<()> {
        fs::create_dir_all(dir).context("create models dir")?;
        fetch_one(
            dir,
            DETECTION_FILE,
            DETECTION_MODEL_URL,
            DETECTION_MODEL_URL_FALLBACK,
            DETECTION_SHA256,
        )?;
        fetch_one(
            dir,
            RECOGNITION_FILE,
            RECOGNITION_MODEL_URL,
            RECOGNITION_MODEL_URL_FALLBACK,
            RECOGNITION_SHA256,
        )?;
        Ok(())
    }

    pub fn fetch_handwriting(dir: &Path) -> Result<()> {
        fs::create_dir_all(dir).context("create models dir")?;
        fetch_one(
            dir,
            HANDWRITING_FILE,
            HANDWRITING_MODEL_URL,
            HANDWRITING_MODEL_URL_FALLBACK,
            HANDWRITING_SHA256,
        )
    }

    fn fetch_one(dir: &Path, name: &str, primary: &str, fallback: &str, sha: &str) -> Result<()> {
        match download_verify(dir, name, primary, sha) {
            Ok(()) => Ok(()),
            Err(primary_err) => download_verify(dir, name, fallback, sha).with_context(|| {
                format!("primary download failed ({primary_err}); fallback {fallback}")
            }),
        }
    }

    fn download_verify(dir: &Path, name: &str, url: &str, sha: &str) -> Result<()> {
        let dest = dir.join(name);
        let tmp = dir.join(format!("{name}.part"));
        let mut resp = ureq::get(url)
            .call()
            .with_context(|| format!("download {url}"))?;
        let mut reader = resp.body_mut().as_reader();
        let mut file = fs::File::create(&tmp).context("create temp model")?;
        let mut hasher = sha2::Sha256::new();
        let mut buf = [0u8; 65536];
        loop {
            let n = reader.read(&mut buf).context("read model")?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n]).context("write model")?;
        }
        file.sync_all().context("sync model")?;
        drop(file);
        let digest = hasher.finalize();
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        if hex != sha {
            let _ = fs::remove_file(&tmp);
            bail!("sha256 mismatch for {name}");
        }
        fs::rename(&tmp, &dest).context("rename model")?;
        verify_file(&dest, sha).map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }
}

#[cfg(feature = "download")]
pub use fetch::{fetch_handwriting, fetch_models};

#[cfg(not(feature = "download"))]
pub fn fetch_models(_dir: &std::path::Path) -> Result<()> {
    Err(Error::Unsupported(
        "model download not enabled; rebuild with feature download".to_owned(),
    ))
}

#[cfg(not(feature = "download"))]
pub fn fetch_handwriting(_dir: &std::path::Path) -> Result<()> {
    Err(Error::Unsupported(
        "model download not enabled; rebuild with feature download".to_owned(),
    ))
}
