use std::path::PathBuf;

use anyhow::{Context, Result};
use docsuite_models::{default_dir, model_paths, status};

pub fn cmd_models_fetch(dir: &std::path::Path) -> Result<()> {
    docsuite_models::fetch_models(dir).context("fetch models")?;
    println!("models ready at {}", dir.display());
    Ok(())
}

pub fn cmd_models_status(dir: &std::path::Path) -> Result<()> {
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

pub fn models_dir(custom: Option<PathBuf>) -> PathBuf {
    custom.unwrap_or_else(default_dir)
}
