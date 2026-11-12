use std::path::Path;
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolStatus {
    pub ffmpeg_available: bool,
    pub ffmpeg_path: Option<String>,
    pub ffmpeg_version: Option<String>,
    pub exiftool_available: bool,
    pub exiftool_path: Option<String>,
    pub exiftool_version: Option<String>,
}

pub fn locate_tools(root_dir: &Path) -> ToolStatus {
    let (ff_ok, ff_path, ff_ver) = check_ffmpeg(root_dir);
    let (ex_ok, ex_path, ex_ver) = check_exiftool(root_dir);

    ToolStatus {
        ffmpeg_available: ff_ok,
        ffmpeg_path: ff_path,
        ffmpeg_version: ff_ver,
        exiftool_available: ex_ok,
        exiftool_path: ex_path,
        exiftool_version: ex_ver,
    }
}

fn check_ffmpeg(root_dir: &Path) -> (bool, Option<String>, Option<String>) {
    // 1. Check local bin folder
    let local_candidate = root_dir.join("bin").join("ffmpeg.exe");
    if local_candidate.exists() {
        if let Some(ver) = get_ffmpeg_ver(&local_candidate) {
            return (true, Some(local_candidate.to_string_lossy().to_string()), Some(ver));
        }
    }

    // 2. Check root folder (same directory as exe)
    let root_candidate = root_dir.join("ffmpeg.exe");
    if root_candidate.exists() {
        if let Some(ver) = get_ffmpeg_ver(&root_candidate) {
            return (true, Some(root_candidate.to_string_lossy().to_string()), Some(ver));
        }
    }

    // 3. Check system PATH
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-version");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    if let Ok(output) = cmd.output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let first_line = text.lines().next().unwrap_or("ffmpeg").to_string();
            let path_str = find_in_path("ffmpeg.exe").unwrap_or_else(|| "ffmpeg (System PATH)".into());
            return (true, Some(path_str), Some(first_line));
        }
    }

    (false, None, None)
}

fn check_exiftool(root_dir: &Path) -> (bool, Option<String>, Option<String>) {
    // 1. Check local bin folder
    let local_candidate = root_dir.join("bin").join("exiftool.exe");
    if local_candidate.exists() {
        if let Some(ver) = get_exiftool_ver(&local_candidate) {
            return (true, Some(local_candidate.to_string_lossy().to_string()), Some(ver));
        }
    }

    // 2. Check root folder (same directory as exe)
    let root_candidate = root_dir.join("exiftool.exe");
    if root_candidate.exists() {
        if let Some(ver) = get_exiftool_ver(&root_candidate) {
            return (true, Some(root_candidate.to_string_lossy().to_string()), Some(ver));
        }
    }

    // 3. Check system PATH
    let mut cmd = Command::new("exiftool");
    cmd.arg("-ver");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    if let Ok(output) = cmd.output() {
        if output.status.success() {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let path_str = find_in_path("exiftool.exe").unwrap_or_else(|| "exiftool (System PATH)".into());
            return (true, Some(path_str), Some(format!("ExifTool v{}", ver)));
        }
    }

    (false, None, None)
}

fn get_ffmpeg_ver(path: &Path) -> Option<String> {
    let mut cmd = Command::new(path);
    cmd.arg("-version");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().ok()?;
    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        Some(text.lines().next().unwrap_or("ffmpeg").to_string())
    } else {
        None
    }
}

fn get_exiftool_ver(path: &Path) -> Option<String> {
    let mut cmd = Command::new(path);
    cmd.arg("-ver");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().ok()?;
    if output.status.success() {
        let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Some(format!("ExifTool v{}", ver))
    } else {
        None
    }
}

fn find_in_path(exe_name: &str) -> Option<String> {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("where.exe");
        cmd.arg(exe_name);
        cmd.creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = cmd.output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                if let Some(first) = text.lines().next() {
                    return Some(first.trim().to_string());
                }
            }
        }
    }
    None
}
