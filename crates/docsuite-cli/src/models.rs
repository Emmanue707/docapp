use std::path::PathBuf;

use anyhow::{Context, Result};
use docsuite_models::{default_dir, model_paths, status};
use docsuite_ocr::handwriting_path;

pub fn cmd_models_fetch(dir: &std::path::Path) -> Result<()> {
    docsuite_models::fetch_models(dir).context("fetch models")?;
    println!("models ready at {}", dir.display());
    Ok(())
}

pub fn cmd_models_fetch_handwriting(dir: &std::path::Path) -> Result<()> {
    docsuite_models::fetch_handwriting(dir).context("fetch handwriting model")?;
    println!("handwriting model ready at {}", dir.display());
    Ok(())
}

pub fn cmd_models_status(dir: &std::path::Path) -> Result<()> {
    let st = status(dir);
    let (det, rec) = model_paths(dir);
    let (htr, _) = handwriting_path(dir);
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
    println!(
        "handwriting: {}",
        if st.handwriting { "present" } else { "missing" }
    );
    if st.handwriting {
        println!("  {}", htr.display());
    }
    Ok(())
}

pub fn models_dir(custom: Option<PathBuf>) -> PathBuf {
    custom.unwrap_or_else(default_dir)
}
