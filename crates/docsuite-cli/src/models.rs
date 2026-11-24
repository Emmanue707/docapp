use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use docsuite_ocr::{
    DETECTION_FILE, DETECTION_SHA256, RECOGNITION_FILE, RECOGNITION_SHA256, default_dir,
    model_paths, status, verify_file,
};
use sha2::Digest;

pub const DETECTION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx";
pub const RECOGNITION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx";

pub fn cmd_models_fetch(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).context("create models dir")?;
    fetch_one(dir, DETECTION_FILE, DETECTION_MODEL_URL, DETECTION_SHA256)?;
    fetch_one(
        dir,
        RECOGNITION_FILE,
        RECOGNITION_MODEL_URL,
        RECOGNITION_SHA256,
    )?;
    println!("models ready at {}", dir.display());
    Ok(())
}

pub fn cmd_models_status(dir: &Path) -> Result<()> {
    let st = status(dir);
    let (det, rec) = model_paths(dir);
    println!("dir: {}", dir.display());
    println!(
        "detection: {}",
        if st.detection { "present" } else { "missing" }
    );
    if st.detection {
        println!("  {}", det.display());
    }
    println!(
        "recognition: {}",
        if st.recognition { "present" } else { "missing" }
    );
    if st.recognition {
        println!("  {}", rec.display());
    }
    Ok(())
}

fn fetch_one(dir: &Path, name: &str, url: &str, sha: &str) -> Result<()> {
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
    let hex = format!("{:x}", hasher.finalize());
    if hex != sha {
        let _ = fs::remove_file(&tmp);
        bail!("sha256 mismatch for {name}");
    }
    fs::rename(&tmp, &dest).context("rename model")?;
    verify_file(&dest, sha).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub fn models_dir(custom: Option<PathBuf>) -> PathBuf {
    custom.unwrap_or_else(default_dir)
}
