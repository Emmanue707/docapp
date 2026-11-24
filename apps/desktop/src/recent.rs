use std::fs;
use std::path::{Path, PathBuf};

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct RecentFile {
    path: String,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct RecentStore {
    files: Vec<RecentFile>,
}

pub fn config_dir() -> PathBuf {
    docsuite_ocr::default_dir()
        .parent()
        .map(|p| p.join("docapp"))
        .unwrap_or_else(|| PathBuf::from(".config/docapp"))
}

fn store_path() -> PathBuf {
    config_dir().join("recent.json")
}

pub fn load_recent() -> Vec<PathBuf> {
    let path = store_path();
    let Ok(data) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let store: RecentStore = serde_json::from_str(&data).unwrap_or_default();
    store
        .files
        .into_iter()
        .map(|f| PathBuf::from(f.path))
        .filter(|p| p.is_file())
        .collect()
}

pub fn push_recent(file: &Path) {
    let _ = fs::create_dir_all(config_dir());
    let mut list = load_recent();
    let canon = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    list.retain(|p| p != &canon);
    list.insert(0, canon);
    list.truncate(12);
    let store = RecentStore {
        files: list
            .into_iter()
            .map(|p| RecentFile {
                path: p.to_string_lossy().into_owned(),
            })
            .collect(),
    };
    if let Ok(json) = serde_json::to_string(&store)
        && let Err(e) = fs::write(store_path(), json)
    {
        let _ = e;
    }
}
