use std::path::Path;
use walkdir::WalkDir;
use serde::{Serialize, Deserialize};

pub const DEFAULT_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "tif", "tiff", "bmp", "heic", "heif", "jxl"
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedFile {
    pub path: String,
    pub filename: String,
    pub size_bytes: u64,
    pub relative_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub root_path: String,
    pub total_files: usize,
    pub total_bytes: u64,
    pub files: Vec<ScannedFile>,
}

pub fn scan_path(target_path_str: &str, recursive: bool) -> ScanResult {
    let target = Path::new(target_path_str);
    if !target.exists() {
        return ScanResult {
            root_path: target_path_str.to_string(),
            total_files: 0,
            total_bytes: 0,
            files: Vec::new(),
        };
    }

    let mut scanned = Vec::new();
    let mut total_bytes = 0u64;

    if target.is_file() {
        if is_supported_image(target) {
            let metadata = target.metadata().ok();
            let size = metadata.map(|m| m.len()).unwrap_or(0);
            total_bytes += size;
            scanned.push(ScannedFile {
                path: target.to_string_lossy().to_string(),
                filename: target.file_name().unwrap_or_default().to_string_lossy().to_string(),
                size_bytes: size,
                relative_path: target.file_name().unwrap_or_default().to_string_lossy().to_string(),
            });
        }
    } else if target.is_dir() {
        let max_depth = if recursive { usize::MAX } else { 1 };
        for entry in WalkDir::new(target)
            .max_depth(max_depth)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.is_file() && is_supported_image(p) {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                total_bytes += size;
                let rel = p.strip_prefix(target).unwrap_or(p);
                scanned.push(ScannedFile {
                    path: p.to_string_lossy().to_string(),
                    filename: p.file_name().unwrap_or_default().to_string_lossy().to_string(),
                    size_bytes: size,
                    relative_path: rel.to_string_lossy().to_string(),
                });
            }
        }
    }

    ScanResult {
        root_path: target_path_str.to_string(),
        total_files: scanned.len(),
        total_bytes,
        files: scanned,
    }
}

pub fn scan_multiple_paths(paths: &[String], recursive: bool) -> ScanResult {
    let mut all_files = Vec::new();
    let mut total_bytes = 0u64;
    let root = paths.first().cloned().unwrap_or_default();

    for p_str in paths {
        let res = scan_path(p_str, recursive);
        total_bytes += res.total_bytes;
        all_files.extend(res.files);
    }

    all_files.sort_by(|a, b| a.path.cmp(&b.path));
    all_files.dedup_by(|a, b| a.path == b.path);

    ScanResult {
        root_path: root,
        total_files: all_files.len(),
        total_bytes,
        files: all_files,
    }
}

fn is_supported_image(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        DEFAULT_EXTENSIONS.contains(&ext_lower.as_str())
    } else {
        false
    }
}
