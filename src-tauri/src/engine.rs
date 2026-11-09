use std::path::Path;
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

use serde::{Serialize, Deserialize};
use crate::metadata::copy_file_timestamps;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub crf: u32,
    pub cpu_used: u32,
    pub pix_fmt: String,
    pub lossless: bool,
}

pub fn get_default_presets() -> Vec<ConversionPreset> {
    vec![
        ConversionPreset {
            id: "anime".into(),
            name: "アニメ / イラスト (4:4:4 10bit - 色にじみ防止)".into(),
            description: "10-bit YUV 4:4:4 - Prevents color bleeding on line art and vibrant colors.".into(),
            crf: 20,
            cpu_used: 6,
            pix_fmt: "yuv444p10le".into(),
            lossless: false,
        },
        ConversionPreset {
            id: "photo".into(),
            name: "写真 / 実写 (4:2:0 10bit - 高圧縮)".into(),
            description: "10-bit YUV 4:2:0 - High compression ratio suitable for photographic images (Fast).".into(),
            crf: 20,
            cpu_used: 8,
            pix_fmt: "yuv420p10le".into(),
            lossless: false,
        },
        ConversionPreset {
            id: "lossless".into(),
            name: "ロスレス (可逆圧縮)".into(),
            description: "Lossless encoding (CRF 0) - Retains original pixel perfection.".into(),
            crf: 0,
            cpu_used: 6,
            pix_fmt: "yuv444p10le".into(),
            lossless: true,
        },
        ConversionPreset {
            id: "custom".into(),
            name: "カスタム".into(),
            description: "User defined quality and parameters.".into(),
            crf: 20,
            cpu_used: 6,
            pix_fmt: "yuv444p10le".into(),
            lossless: false,
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionOptions {
    pub preset_id: String,
    pub crf: u32,
    pub cpu_used: u32,
    pub thread_count: usize,
    pub preserve_metadata: bool,
    pub preserve_timestamps: bool,
    pub trash_originals: bool,
    pub output_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConvertResult {
    pub src_path: String,
    pub dst_path: String,
    pub filename: String,
    pub success: bool,
    pub original_bytes: u64,
    pub converted_bytes: u64,
    pub command: String,
    pub error_message: Option<String>,
    pub metadata_copied: bool,
    pub timestamps_copied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEventPayload {
    pub current_index: usize,
    pub total_files: usize,
    pub percent: f32,
    pub current_file: String,
    pub saved_bytes: i64,
    pub last_result: Option<FileConvertResult>,
    pub batch_message: Option<String>,
}

pub fn convert_single_file(
    ffmpeg_cmd: &str,
    src: &Path,
    dst: &Path,
    options: &ConversionOptions,
) -> FileConvertResult {
    let filename = src.file_name().unwrap_or_default().to_string_lossy().to_string();
    let orig_bytes = src.metadata().map(|m| m.len()).unwrap_or(0);

    let pix_fmt = if options.preset_id == "photo" {
        "yuv420p10le"
    } else {
        "yuv444p10le"
    };

    // 静止画エンコード用最適化パラメータ (先読み・GOPバッファを最小化してメモリ消費を激減＆8K対応)
    let mut svt_params = vec![
        "lookahead=0".to_string(),
        "keyint=1".to_string(),
        "lp=1".to_string(),
    ];

    if options.preset_id == "lossless" || options.crf == 0 {
        svt_params.push("lossless=1".to_string());
    } else if options.preset_id == "anime" {
        svt_params.push("tune=0".to_string());
    } else if options.preset_id == "photo" {
        svt_params.push("tune=0".to_string());
        svt_params.push("enable-restoration=1".to_string());
    }

    // Build ffmpeg command
    let mut cmd = Command::new(ffmpeg_cmd);
    let args: Vec<String> = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-i".into(),
        src.to_string_lossy().to_string(),
        "-frames:v".into(),
        "1".into(),
        "-c:v".into(),
        "libsvtav1".into(),
        "-crf".into(),
        options.crf.to_string(),
        "-preset".into(),
        options.cpu_used.to_string(),
        "-svtav1-params".into(),
        svt_params.join(":"),
        "-pix_fmt".into(),
        pix_fmt.into(),
        dst.to_string_lossy().to_string(),
    ];

    cmd.args(&args);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let full_cmd_str = format!("{} {}", ffmpeg_cmd, args.join(" "));

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            return FileConvertResult {
                src_path: src.to_string_lossy().to_string(),
                dst_path: dst.to_string_lossy().to_string(),
                filename,
                success: false,
                original_bytes: orig_bytes,
                converted_bytes: 0,
                command: full_cmd_str,
                error_message: Some(format!("Failed to spawn ffmpeg: {}", e)),
                metadata_copied: false,
                timestamps_copied: false,
            };
        }
    };

    if !output.status.success() || !dst.exists() {
        let err_str = String::from_utf8_lossy(&output.stderr).to_string();
        return FileConvertResult {
            src_path: src.to_string_lossy().to_string(),
            dst_path: dst.to_string_lossy().to_string(),
            filename,
            success: false,
            original_bytes: orig_bytes,
            converted_bytes: 0,
            command: full_cmd_str,
            error_message: Some(if err_str.is_empty() { "Conversion failed".into() } else { err_str }),
            metadata_copied: false,
            timestamps_copied: false,
        };
    }

    let conv_bytes = dst.metadata().map(|m| m.len()).unwrap_or(0);

    // Timestamps preservation (fast filetime)
    let time_ok = if options.preserve_timestamps {
        copy_file_timestamps(src, dst).is_ok()
    } else {
        false
    };

    FileConvertResult {
        src_path: src.to_string_lossy().to_string(),
        dst_path: dst.to_string_lossy().to_string(),
        filename,
        success: true,
        original_bytes: orig_bytes,
        converted_bytes: conv_bytes,
        command: full_cmd_str,
        error_message: None,
        metadata_copied: options.preserve_metadata,
        timestamps_copied: time_ok,
    }
}

pub fn move_files_to_trash(paths: &[String]) -> Result<usize, String> {
    let mut count = 0;
    for p_str in paths {
        let p = Path::new(p_str);
        if p.exists() {
            if trash::delete(p).is_ok() {
                count += 1;
            }
        }
    }
    Ok(count)
}
