use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
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

/// 探索不要な隠しフォルダやキャッシュ・システムディレクトリを判定
fn is_ignored_dir(entry: &walkdir::DirEntry) -> bool {
    if entry.file_type().is_dir() {
        if let Some(name) = entry.file_name().to_str() {
            // カレントディレクトリ以外のドットから始まる隠しフォルダ（.git, .svn, .cache 等）
            if name.starts_with('.') && name != "." {
                return true;
            }
            let lower = name.to_lowercase();
            if matches!(
                lower.as_str(),
                "node_modules"
                    | "target"
                    | "$recycle.bin"
                    | "system volume information"
                    | "recovery"
                    | "windows"
            ) {
                return true;
            }
        }
    }
    false
}

pub fn scan_path_with_progress<F>(
    target_path_str: &str,
    recursive: bool,
    cancel_flag: &AtomicBool,
    mut on_progress: F,
) -> Option<ScanResult>
where
    F: FnMut(usize, u64),
{
    let target = Path::new(target_path_str);
    if !target.exists() {
        return Some(ScanResult {
            root_path: target_path_str.to_string(),
            total_files: 0,
            total_bytes: 0,
            files: Vec::new(),
        });
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
            on_progress(scanned.len(), total_bytes);
        }
    } else if target.is_dir() {
        let max_depth = if recursive { usize::MAX } else { 1 };
        let mut last_reported = std::time::Instant::now();

        let walker = WalkDir::new(target)
            .max_depth(max_depth)
            .into_iter()
            .filter_entry(|e| !is_ignored_dir(e));

        for entry in walker.filter_map(|e| e.ok()) {
            if cancel_flag.load(Ordering::Relaxed) {
                return None;
            }

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

                // 200件ごと、または80ms以上経過していたら進捗通知
                if scanned.len() % 200 == 0 || last_reported.elapsed().as_millis() > 80 {
                    on_progress(scanned.len(), total_bytes);
                    last_reported = std::time::Instant::now();
                }
            }
        }
        on_progress(scanned.len(), total_bytes);
    }

    Some(ScanResult {
        root_path: target_path_str.to_string(),
        total_files: scanned.len(),
        total_bytes,
        files: scanned,
    })
}

pub fn scan_multiple_paths_with_progress<F>(
    paths: &[String],
    recursive: bool,
    cancel_flag: &AtomicBool,
    mut on_progress: F,
) -> Option<ScanResult>
where
    F: FnMut(usize, u64),
{
    let mut all_files = Vec::new();
    let mut total_bytes = 0u64;
    let root = paths.first().cloned().unwrap_or_default();

    for p_str in paths {
        if cancel_flag.load(Ordering::Relaxed) {
            return None;
        }
        let res = scan_path_with_progress(p_str, recursive, cancel_flag, |count, bytes| {
            on_progress(all_files.len() + count, total_bytes + bytes);
        })?;
        total_bytes += res.total_bytes;
        all_files.extend(res.files);
    }

    all_files.sort_by(|a, b| a.path.cmp(&b.path));
    all_files.dedup_by(|a, b| a.path == b.path);

    Some(ScanResult {
        root_path: root,
        total_files: all_files.len(),
        total_bytes,
        files: all_files,
    })
}

#[allow(dead_code)]
pub fn scan_path(target_path_str: &str, recursive: bool) -> ScanResult {
    let dummy_cancel = AtomicBool::new(false);
    scan_path_with_progress(target_path_str, recursive, &dummy_cancel, |_, _| {})
        .unwrap_or(ScanResult {
            root_path: target_path_str.to_string(),
            total_files: 0,
            total_bytes: 0,
            files: Vec::new(),
        })
}

#[allow(dead_code)]
pub fn scan_multiple_paths(paths: &[String], recursive: bool) -> ScanResult {
    let dummy_cancel = AtomicBool::new(false);
    scan_multiple_paths_with_progress(paths, recursive, &dummy_cancel, |_, _| {})
        .unwrap_or(ScanResult {
            root_path: paths.first().cloned().unwrap_or_default(),
            total_files: 0,
            total_bytes: 0,
            files: Vec::new(),
        })
}

fn is_supported_image(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        DEFAULT_EXTENSIONS.contains(&ext_lower.as_str())
    } else {
        false
    }
}
